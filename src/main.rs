//! sort_pictures - Automatic photo and video organization by date.
//!
//! This daemon monitors directories for new photos and videos, extracts dates
//! from EXIF metadata or filenames, and organizes them into a hierarchical
//! directory structure.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod config;
mod daemon;
mod date;
mod files;
mod gps;
mod path;

use config::{Config, Dir, Place, load_config};
use gps::find_matching_place;

/// Global configuration, protected by mutex for thread-safe access in daemon mode.
static GLOBAL_PARAMS: Mutex<Config> = Mutex::new(Config::empty());

fn main() -> Result<(), Box<dyn Error>> {
    let binding = &GLOBAL_PARAMS;
    let mut cli = binding.lock()?;
    *cli = load_config()?;

    // Print configuration
    print_config(&cli)?;

    if cli.dirs.is_empty() {
        println!("Please specify dirs to watch in config.toml.");
        return Ok(());
    }

    // Process existing files in all directories
    for dir in &cli.dirs {
        let source = dir.source.clone().ok_or("Cannot unwrap source")?;
        process_directory(&source.canonicalize()?, dir, &cli.places)?;
    }

    if !cli.daemonize {
        println!("Finished!");
        return Ok(());
    }

    // Start daemon mode
    let places = cli.places.clone();
    let dirs = cli.dirs.clone();
    drop(cli); // Release lock before spawning threads

    daemon::run_daemon(
        &dirs,
        |dir| {
            let places = places.clone();
            let target = dir.target.clone();
            let nodecade = dir.nodecade;
            let noyear = dir.noyear;
            let nomonth = dir.nomonth;

            move |path: &Path| {
                if let Err(e) = process_file(path, &target, nodecade, noyear, nomonth, &places) {
                    eprintln!("Error processing {:?}: {}", path, e);
                }
            }
        },
        daemon::DaemonConfig::default(),
    );

    println!("Finished!");
    Ok(())
}

/// Prints the current configuration to stdout.
fn print_config(config: &Config) -> Result<(), Box<dyn Error>> {
    for dir in &config.dirs {
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

    for place in &config.places {
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

    Ok(())
}

/// Process all files in a directory.
fn process_directory(
    source_dir: &Path,
    dir_config: &Dir,
    places: &[Place],
) -> Result<(), Box<dyn Error>> {
    let files = files::scan_directory(source_dir)?;

    for file_path in files {
        if let Err(e) = process_file(
            &file_path,
            &dir_config.target,
            dir_config.nodecade,
            dir_config.noyear,
            dir_config.nomonth,
            places,
        ) {
            eprintln!("Error processing {:?}: {}", file_path, e);
        }
    }

    Ok(())
}

/// Process a single file: extract date, check GPS, move to target.
fn process_file(
    file_path: &Path,
    default_target: &Option<PathBuf>,
    nodecade: bool,
    noyear: bool,
    nomonth: bool,
    places: &[Place],
) -> Result<(), Box<dyn Error>> {
    // Extract date from file
    let extracted = match date::extract_date(file_path) {
        Some(d) => d,
        None => return Ok(()), // No date found, skip silently
    };

    // Determine target based on GPS location or default
    let (target_dir, use_nodecade, use_noyear, use_nomonth) = determine_target(
        file_path,
        &extracted,
        default_target,
        nodecade,
        noyear,
        nomonth,
        places,
    )?;

    // Build the full target path with date hierarchy
    let final_target = files::build_target_path(
        &target_dir,
        &extracted.date,
        use_nodecade,
        use_noyear,
        use_nomonth,
    );

    // Get the filename
    let filename = file_path
        .file_name()
        .ok_or("Cannot get file name")?
        .to_string_lossy();

    // Move the file
    files::move_file_safe(file_path, &final_target, &filename)?;

    Ok(())
}

/// Determine the target directory and hierarchy settings for a file.
///
/// Checks GPS data against configured places first, then falls back to default target.
fn determine_target(
    file_path: &Path,
    extracted: &date::ExtractedDate,
    default_target: &Option<PathBuf>,
    default_nodecade: bool,
    default_noyear: bool,
    default_nomonth: bool,
    places: &[Place],
) -> Result<(PathBuf, bool, bool, bool), Box<dyn Error>> {
    // Check if GPS matches a configured place
    if let Some(ref gps) = extracted.gps
        && let Some(place_match) = find_matching_place(gps, places)
    {
        println!(
            "Found picture from {:?}, distance: {:.2}km",
            place_match.place.name, place_match.distance_km
        );

        let target = if let Some(ref t) = place_match.place.target {
            t.canonicalize()?
        } else {
            file_path.parent().ok_or("Cannot get parent")?.to_path_buf()
        };

        return Ok((
            target,
            place_match.place.nodecade,
            place_match.place.noyear,
            place_match.place.nomonth,
        ));
    }

    // Use default target
    let target = if let Some(t) = default_target {
        t.canonicalize()?
    } else {
        file_path.parent().ok_or("Cannot get parent")?.to_path_buf()
    };

    Ok((target, default_nodecade, default_noyear, default_nomonth))
}
