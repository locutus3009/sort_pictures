//! sort_pictures - Automatic photo and video organization by date.
//!
//! This daemon monitors directories for new photos and videos, extracts dates
//! from EXIF metadata or filenames, and organizes them into a hierarchical
//! directory structure.

use chrono::Datelike;
use chrono::NaiveDate;
use nom_exif::GPSInfo;
use nom_exif::{Exif, ExifIter, ExifTag, MediaParser, MediaSource, TrackInfo, TrackInfoTag};
use notify::event::{CreateKind, ModifyKind, RenameMode};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use regex::Regex;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;

mod config;
mod gps;
mod path;

use config::{Config, Place, load_config};
use gps::find_matching_place;

/// Global configuration, protected by mutex for thread-safe access in daemon mode.
static GLOBAL_PARAMS: Mutex<Config> = Mutex::new(Config::empty());

/// Processes a file or directory, extracting dates and organizing files.
///
/// # Arguments
/// * `nodecade` - Skip decade folder creation
/// * `noyear` - Skip year folder creation
/// * `nomonth` - Skip month folder creation
/// * `fname` - Path to process (file or directory)
/// * `target_dir` - Target directory for organized files
/// * `places` - GPS-based place configurations
fn process_fname(
    nodecade: bool,
    noyear: bool,
    nomonth: bool,
    mut fname: PathBuf,
    target_dir: &Option<PathBuf>,
    places: &[Place],
) -> Result<(), Box<dyn Error>> {
    // Create regex patterns for various date formats
    let yyyy_mm_dd_prefix_regex = Regex::new(r"^(\d{4}[-_]\d{2}[-_]\d{2})")?;
    let yyyy_mm_dd_embedded_regex = Regex::new(r"[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})")?;
    let yyyymmdd_regex = Regex::new(r"(\d{8})")?;
    let yyyy_mmdd_regex = Regex::new(r"(\d{4})_(\d{4})")?;

    // Storage for file info and dates
    let mut file_date_map: Vec<(PathBuf, String, PathBuf, bool, bool, bool)> = Vec::new();

    let mut paths: Vec<PathBuf> = Vec::new();

    // First pass: collect file information
    if fname.is_dir() {
        let entries = fs::read_dir(&fname)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip directories and hidden files
            if path.is_dir()
                || path
                    .file_name()
                    .ok_or("Cannot get file name")?
                    .to_string_lossy()
                    .starts_with(".")
                || path
                    .extension()
                    .is_some_and(|ext| ext == "sh" || ext == "rs")
            {
                continue;
            }

            paths.push(path.clone());
        }
    }

    if fname.is_file() {
        paths.push(fname.clone());
        fname = fname.parent().ok_or("Cannot get parent dir")?.to_path_buf();
    }

    let mut parser = MediaParser::new();
    for path in paths {
        let filename = path
            .file_name()
            .ok_or("Cannot get file name")?
            .to_string_lossy();
        let mut date_found = None;
        let mut gps_data = None;

        // Try to read EXIF from image files
        let msr = MediaSource::file_path(&path);
        if let Ok(ms) = msr {
            if ms.has_exif() {
                if let Some((exif_date, gps_info)) = parse_exif(&mut parser, ms) {
                    date_found = Some(exif_date);
                    gps_data = gps_info;
                }
            } else if ms.has_track()
                && let Some((exif_date, gps_info)) = parse_track(&mut parser, ms)
            {
                date_found = Some(exif_date);
                gps_data = gps_info;
            }
        }

        // If EXIF failed, try filename patterns
        if date_found.is_none() {
            // Check YYYY-MM-DD format at start of filename
            if let Some(captures) = yyyy_mm_dd_prefix_regex.captures(&filename) {
                let date_str = captures
                    .get(1)
                    .ok_or("Cannot get captures")?
                    .as_str()
                    .to_string()
                    .replace("_", "-");
                if is_valid_date(&date_str) {
                    date_found = Some(date_str);
                }
            }
        }

        // Check YYYY-MM-DD format in middle of string
        if date_found.is_none() {
            let padded_filename = format!(" {}", filename);
            if let Some(captures) = yyyy_mm_dd_embedded_regex.captures(&padded_filename) {
                let date_str = captures
                    .get(1)
                    .ok_or("Cannot get captures")?
                    .as_str()
                    .to_string()
                    .replace("_", "-");
                if is_valid_date(&date_str) {
                    date_found = Some(date_str);
                }
            }
        }

        // Check YYYY_MMDD format (e.g., 2020_0718_064509.MP4)
        if date_found.is_none()
            && let Some(captures) = yyyy_mmdd_regex.captures(&filename)
        {
            let year = captures.get(1).ok_or("Cannot get captures")?.as_str();
            let mmdd = captures.get(2).ok_or("Cannot get captures")?.as_str();

            if let Some(date_str) = try_parse_yyyy_mmdd(year, mmdd) {
                date_found = Some(date_str);
            }
        }

        // Check YYYYMMDD format anywhere in filename
        if date_found.is_none()
            && let Some(captures) = yyyymmdd_regex.captures(&filename)
        {
            let date_part = captures.get(1).ok_or("Cannot get captures")?.as_str();
            if let Some(date_str) = try_parse_yyyymmdd(date_part) {
                date_found = Some(date_str);
            }
        }

        // Check GPS data against configured places
        let mut into_place = false;
        if let Some(ref gps) = gps_data
            && let Some(place_match) = find_matching_place(gps, places)
        {
            println!(
                "Found picture from {:?}, distance: {:.2}km",
                place_match.place.name, place_match.distance_km
            );
            file_date_map.push((
                path.to_owned(),
                date_found.clone().unwrap(),
                if let Some(d) = &place_match.place.target {
                    d.clone().canonicalize()?
                } else {
                    fname.clone()
                },
                place_match.place.nodecade,
                place_match.place.noyear,
                place_match.place.nomonth,
            ));
            into_place = true;
        }

        if !into_place && let Some(date) = date_found {
            file_date_map.push((
                path.to_owned(),
                date,
                if let Some(d) = &target_dir {
                    d.clone().canonicalize()?
                } else {
                    fname.clone()
                },
                nodecade,
                noyear,
                nomonth,
            ));
        }
    }

    // Move files to their date-based directories
    for (file_path, file_date, target_dir, nodecade, noyear, nomonth) in &file_date_map {
        let parts: Vec<&str> = file_date.split('-').collect();
        let year_str = parts[0];
        let month = parts[1];

        let year = year_str.parse::<i32>()?;

        // Calculate decade range
        let decade_start = (year / 10) * 10;
        let decade_end = decade_start + 9;
        let decade_range = &format!("{}-{}", decade_start, decade_end);

        // Build target directory path
        let mut date_dir = target_dir.clone();

        if !nodecade {
            date_dir = date_dir.join(decade_range);
        }

        if !noyear {
            date_dir = date_dir.join(year_str);
        }

        if !nomonth {
            date_dir = date_dir.join(month);
        }

        date_dir = date_dir.join(file_date);
        if !date_dir.exists() {
            fs::create_dir_all(&date_dir)?;
        }

        if file_path.exists() {
            let filename = file_path.file_name().ok_or("Cannot get file name")?;
            let target_path = {
                let original_path = date_dir.join(filename);
                if !original_path.exists() {
                    original_path
                } else {
                    // Handle filename collisions with -N suffix
                    let stem = original_path
                        .file_stem()
                        .ok_or("Cannot get file name non-extension portion")?
                        .to_string_lossy();
                    let extension = original_path
                        .extension()
                        .map(|ext| format!(".{}", ext.to_string_lossy()))
                        .unwrap_or_default();

                    let mut counter = 1;
                    loop {
                        let new_name = format!("{}-{}{}", stem, counter, extension);
                        let new_path = date_dir.join(new_name);
                        if !new_path.exists() {
                            break new_path;
                        }
                        counter += 1;
                    }
                }
            };

            let source = fname.join(filename);
            let target = target_path.clone();
            let (base, rel_source, rel_target) = path::find_common_base(&source, &target);

            // Small delay for filesystem sync
            std::thread::sleep(std::time::Duration::from_millis(1000));
            print!(
                "Move: \"{}\"/{{\"{}\" -> \"{}\"}}... ",
                base.to_string_lossy(),
                rel_source.to_string_lossy(),
                rel_target.to_string_lossy()
            );
            io::stdout().flush()?;

            match fs::rename(file_path, &target_path) {
                Ok(_) => println!("success"),
                Err(e) => println!("error: {}", e),
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let binding = &GLOBAL_PARAMS;
    let mut cli = binding.lock()?;
    *cli = load_config()?;

    for dir in &cli.dirs {
        println!(
            "Watch source: \"{}\"",
            dir.source.clone().ok_or("Cannot unwrap")?.to_string_lossy()
        );
        println!(
            "      target: \"{}\"",
            dir.target
                .clone()
                .unwrap_or(dir.source.clone().ok_or("Cannot unwrap")?)
                .to_string_lossy()
        );
        println!("      create decade dir: {}", !dir.nodecade);
        println!("      create year dir:   {}", !dir.noyear);
        println!("      create month dir:  {}", !dir.nomonth);
    }

    for place in &cli.places {
        println!("Watch place: \"{}\"", place.name);
        println!(
            "      target: \"{}\"",
            place
                .target
                .clone()
                .ok_or("Cannot unwrap")?
                .to_string_lossy()
        );
        println!("      create decade dir: {}", !place.nodecade);
        println!("      create year dir:   {}", !place.noyear);
        println!("      create month dir:  {}", !place.nomonth);
    }

    if cli.dirs.is_empty() {
        println!("Please specify dirs to watch in config.toml.");
        return Ok(());
    }

    for dir in &cli.dirs {
        process_fname(
            dir.nodecade,
            dir.noyear,
            dir.nomonth,
            dir.source.clone().ok_or("Cannot unwrap")?.canonicalize()?,
            &dir.target,
            &cli.places,
        )?;
    }

    if !cli.daemonize {
        println!("Finished!");
        return Ok(());
    }

    let mut tokens: Vec<_> = Vec::new();
    for dir in &cli.dirs {
        let nodecade = dir.nodecade;
        let noyear = dir.noyear;
        let nomonth = dir.nomonth;
        let path = dir.source.clone().unwrap();
        let target = dir.target.clone();
        let places = cli.places.clone();

        let token = thread::spawn(move || {
            let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();

            println!("Watch: \"{}\"", path.display());

            let mut watcher = notify::recommended_watcher(tx).unwrap();

            watcher
                .watch(&path.canonicalize().unwrap(), RecursiveMode::NonRecursive)
                .unwrap();

            for res in rx {
                match res {
                    Ok(event) => match event.kind {
                        EventKind::Create(CreateKind::File) => {
                            let path = &event.paths[0];
                            if path.is_dir()
                                || path.file_name().unwrap().to_string_lossy().starts_with(".")
                                || path
                                    .extension()
                                    .is_some_and(|ext| ext == "sh" || ext == "rs")
                            {
                                println!("Skip: \"{}\"", path.display());
                                continue;
                            }

                            process_fname(
                                nodecade,
                                noyear,
                                nomonth,
                                path.to_path_buf(),
                                &target,
                                &places,
                            )
                            .unwrap();
                        }
                        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                            let path = &event.paths[0];
                            if path.is_dir()
                                || path.file_name().unwrap().to_string_lossy().starts_with(".")
                                || path
                                    .extension()
                                    .is_some_and(|ext| ext == "sh" || ext == "rs")
                            {
                                println!("Skip: \"{}\"", path.display());
                                continue;
                            }

                            process_fname(
                                nodecade,
                                noyear,
                                nomonth,
                                path.to_path_buf(),
                                &target,
                                &places,
                            )
                            .unwrap();
                        }
                        _ => (),
                    },
                    Err(e) => println!("watch error: {:?}", e),
                }
            }
        });
        tokens.push(token);
    }

    for token in tokens {
        token.join().unwrap();
    }

    println!("Finished!");
    Ok(())
}

/// Extracts date and GPS info from EXIF metadata.
fn parse_exif<T: std::io::Read + std::io::Seek>(
    parser: &mut MediaParser,
    ms: MediaSource<T>,
) -> Option<(String, Option<GPSInfo>)> {
    let iter: ExifIter = match parser.parse(ms) {
        Ok(p) => p,
        Err(e) => {
            println!("Cannot parse: {}", e);
            return None;
        }
    };
    let exif: Exif = iter.into();

    // Priority order for date tags
    let date_tags = [
        ExifTag::DateTimeOriginal,
        ExifTag::CreateDate,
        ExifTag::ModifyDate,
    ];

    let mut result_date = None;
    for &tag in &date_tags {
        if let Some(field) = exif.get(tag) {
            let time_opt = match field {
                nom_exif::EntryValue::Time(dt) => Some((dt.year(), dt.month(), dt.day())),
                nom_exif::EntryValue::NaiveDateTime(dt) => Some((dt.year(), dt.month(), dt.day())),
                _ => field.as_time().map(|dt| (dt.year(), dt.month(), dt.day())),
            };

            if let Some((year, month, day)) = time_opt {
                result_date = Some((
                    format!("{:04}-{:02}-{:02}", year, month, day),
                    exif.get_gps_info().ok().flatten(),
                ));
                break;
            }
        }
    }

    result_date
}

/// Extracts date and GPS info from video track metadata.
fn parse_track<T: std::io::Read + std::io::Seek>(
    parser: &mut MediaParser,
    ms: MediaSource<T>,
) -> Option<(String, Option<GPSInfo>)> {
    let track: TrackInfo = match parser.parse(ms) {
        Ok(p) => p,
        Err(e) => {
            println!("Cannot parse: {}", e);
            return None;
        }
    };

    let date_tags = [TrackInfoTag::CreateDate];

    let mut result_date = None;
    for &tag in &date_tags {
        if let Some(field) = track.get(tag) {
            let time = field.as_time().unwrap();
            result_date = Some((
                format!("{:04}-{:02}-{:02}", time.year(), time.month(), time.day()),
                track.get_gps_info().cloned(),
            ));
            break;
        }
    }

    result_date
}

/// Validates a date string in YYYY-MM-DD format.
fn is_valid_date(date_str: &str) -> bool {
    if date_str.len() != 10 {
        return false;
    }

    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return false;
    }

    let year = parts[0].parse::<i32>().unwrap_or(0);
    let month = parts[1].parse::<u32>().unwrap_or(0);
    let day = parts[2].parse::<u32>().unwrap_or(0);

    if !(1990..=2099).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }

    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Attempts to parse YYYYMMDD format to YYYY-MM-DD.
fn try_parse_yyyymmdd(date_str: &str) -> Option<String> {
    if date_str.len() != 8 {
        return None;
    }

    let year = &date_str[0..4];
    let month = &date_str[4..6];
    let day = &date_str[6..8];

    let year_num = year.parse::<i32>().ok()?;
    let month_num = month.parse::<u32>().ok()?;
    let day_num = day.parse::<u32>().ok()?;

    if !(1990..=2099).contains(&year_num)
        || !(1..=12).contains(&month_num)
        || !(1..=31).contains(&day_num)
    {
        return None;
    }

    NaiveDate::from_ymd_opt(year_num, month_num, day_num)?;

    Some(format!("{}-{}-{}", year, month, day))
}

/// Attempts to parse YYYY_MMDD format to YYYY-MM-DD.
fn try_parse_yyyy_mmdd(year: &str, mmdd: &str) -> Option<String> {
    if mmdd.len() != 4 {
        return None;
    }

    let month = &mmdd[0..2];
    let day = &mmdd[2..4];

    let year_num = year.parse::<i32>().ok()?;
    let month_num = month.parse::<u32>().ok()?;
    let day_num = day.parse::<u32>().ok()?;

    if !(1990..=2099).contains(&year_num)
        || !(1..=12).contains(&month_num)
        || !(1..=31).contains(&day_num)
    {
        return None;
    }

    NaiveDate::from_ymd_opt(year_num, month_num, day_num)?;

    Some(format!("{}-{}-{}", year, month, day))
}
