use chrono::Datelike;
use chrono::NaiveDate;
use clap::{CommandFactory, Parser};
use nom_exif::{
    Exif, ExifIter, ExifTag, GPSInfo, MediaParser, MediaSource, TrackInfo, TrackInfoTag,
};
use notify::event::{CreateKind, ModifyKind, RenameMode};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

mod path;

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Dir {
    /// Source directory to monitor files.
    #[serde(default)]
    source: Option<PathBuf>,
    /// Target directory to save files.
    #[serde(default)]
    target: Option<PathBuf>,
    #[serde(default)]
    nodecade: bool,
    /// Disable creation of "year" directory.
    #[serde(default)]
    noyear: bool,
    /// Disable creation of "month" directory.
    #[serde(default)]
    nomonth: bool,
}

#[derive(Parser, Deserialize, Serialize, Clone, Debug)]
#[command(name = "sort_pictures")]
#[command(about = "A program to re-order pictures in the directory")]
struct Config {
    /// Config file path (default: ~/.config/sort_pictures/config.toml)
    #[arg(short, long)]
    #[serde(skip)]
    config: Option<PathBuf>,

    /// Run as a daemon.
    #[arg(long)]
    #[serde(skip)]
    daemonize: bool,

    /// Directory configurations
    #[arg(skip)]
    #[serde(default)]
    dirs: Vec<Dir>,
}

impl Config {
    const fn empty() -> Self {
        Self {
            config: None,
            daemonize: false,
            dirs: Vec::new(),
        }
    }
}

static GLOBAL_PARAMS: Mutex<Config> = Mutex::new(Config::empty());

fn process_fname(
    nodecade: bool,
    noyear: bool,
    nomonth: bool,
    mut fname: PathBuf,
    target_dir: &Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    // Создаем регулярные выражения для различных форматов дат
    let yyyy_mm_dd_prefix_regex = Regex::new(r"^(\d{4}[-_]\d{2}[-_]\d{2})")?;
    let yyyy_mm_dd_embedded_regex = Regex::new(r"[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})")?;
    let yyyymmdd_regex = Regex::new(r"(\d{8})")?;
    let yyyy_mmdd_regex = Regex::new(r"(\d{4})_(\d{4})")?;

    // Множество для хранения уникальных дат в формате YYYY-MM-DD
    let mut date_dirs: HashSet<String> = HashSet::new();

    // Словарь для хранения информации о файлах и их датах
    let mut file_date_map: Vec<(PathBuf, String)> = Vec::new();

    //    println!("Обрабатываю путь: {}", fname.display());

    let mut paths: Vec<PathBuf> = Vec::new();

    // Первый проход: собираем информацию о файлах и датах
    if fname.is_dir() {
        let entries = fs::read_dir(&fname)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Пропускаем директории и скрытые файлы
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

        // Проверяем, является ли файл изображением, и пытаемся прочитать EXIF

        let msr = MediaSource::file_path(&path);
        if let Ok(ms) = msr {
            if ms.has_exif() {
                if let Some((exif_date, gps_info)) = parse_exif(&mut parser, ms) {
                    //                let exif_date_clone = exif_date.clone();
                    date_found = Some(exif_date);
                    gps_data = gps_info;
                }
            } else if ms.has_track() {
                if let Some((exif_date, gps_info)) = parse_track(&mut parser, ms) {
                    //                let exif_date_clone = exif_date.clone();
                    date_found = Some(exif_date);
                    gps_data = gps_info;
                }
            }
        }

        // Если EXIF не сработал, применяем анализ имени файла
        if date_found.is_none() {
            // Проверяем формат YYYY-MM-DD в начале файла
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

        // Проверяем формат YYYY-MM-DD в середине строки
        if date_found.is_none() {
            let padded_filename = format!(" {}", filename);
            if let Some(captures) = yyyy_mm_dd_embedded_regex.captures(&padded_filename) {
                let date_str = captures
                    .get(1)
                    .ok_or("Cannot get capturese")?
                    .as_str()
                    .to_string()
                    .replace("_", "-");
                if is_valid_date(&date_str) {
                    date_found = Some(date_str);
                }
            }
        }

        // Проверяем формат YYYY_MMDD (как в 2020_0718_064509_034.MP4)
        if date_found.is_none() {
            if let Some(captures) = yyyy_mmdd_regex.captures(&filename) {
                let year = captures.get(1).ok_or("Cannot get captures")?.as_str();
                let mmdd = captures.get(2).ok_or("Cannot get captures")?.as_str();

                if let Some(date_str) = try_parse_yyyy_mmdd(year, mmdd) {
                    date_found = Some(date_str);
                }
            }
        }

        // Проверяем формат YYYYMMDD где-либо в имени файла
        if date_found.is_none() {
            if let Some(captures) = yyyymmdd_regex.captures(&filename) {
                let date_part = captures.get(1).ok_or("Cannot get captures")?.as_str();
                if let Some(date_str) = try_parse_yyyymmdd(date_part) {
                    date_found = Some(date_str);
                }
            }
        }

        if let Some(gps) = gps_data {
            println!("Position: {:?}", gps.format_iso6709(),);
        }

        if let Some(date) = date_found {
            date_dirs.insert(date.clone());
            file_date_map.push((path.to_owned(), date));
        }
    }

    // Выводим найденные даты
    let mut sorted_dates: Vec<&String> = date_dirs.iter().collect();
    sorted_dates.sort();

    if sorted_dates.is_empty() {
        //        println!("Files with date not found");
        return Ok(());
    }

    /*
        println!("Найдены следующие даты:");
        for date in &sorted_dates {
            println!("- {}", date);
    }
    */

    // Создаем директории и перемещаем файлы
    for date in sorted_dates {
        //        println!("Обрабатываю дату: {}", date);

        let parts: Vec<&str> = date.split('-').collect();
        let year_str = parts[0];
        let month = parts[1];

        // Преобразуем строку в число
        let year = year_str.parse::<i32>()?;

        // Вычисляем начало и конец десятилетия
        let decade_start = (year / 10) * 10; // Округляем до начала десятилетия
        let decade_end = decade_start + 9;

        // Форматируем результат
        let decade_range = &format!("{}-{}", decade_start, decade_end);

        // Создаем директорию для даты, если она еще не существует
        let mut date_dir = if let Some(d) = &target_dir {
            d.clone().canonicalize()?
        } else {
            fname.clone()
        };

        if !nodecade {
            date_dir = date_dir.join(decade_range);
        }

        if !noyear {
            date_dir = date_dir.join(year_str);
        }

        if !nomonth {
            date_dir = date_dir.join(month);
        }

        date_dir = date_dir.join(date);
        if !date_dir.exists() {
            fs::create_dir_all(&date_dir)?;
        }

        // Перемещаем файлы, соответствующие текущей дате
        for (file_path, file_date) in &file_date_map {
            if file_date == date && file_path.exists() {
                let filename = file_path.file_name().ok_or("Cannot get file name")?;
                let target_path = {
                    let original_path = date_dir.join(filename);
                    if !original_path.exists() {
                        original_path
                    } else {
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

        /*
        println!(
                "Файлы с датой {} перемещены в директорию {}/{}/{}/{}",
                date, decade_range, year, month, date
        );
         */
    }

    Ok(())
}

fn load_config() -> Result<Config, Box<dyn Error>> {
    // Parse CLI first to get config file path
    let cli_args = Config::parse();

    let config_path = match &cli_args.config {
        Some(path) => path.clone(),
        None => {
            let mut path = dirs::config_dir().ok_or("Cannot get config dir")?;
            path.push("sort_pictures");
            std::fs::create_dir_all(&path)?;
            path.push("config.toml");
            path
        }
    };

    // Load config from file
    let mut config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        toml::from_str::<Config>(&content)?
    } else {
        println!("Config file not found at {:?}, using defaults", config_path);
        Config::empty()
    };

    // Now we need to check which CLI args were actually provided
    // and override only those in the config
    let matches = Config::command().get_matches();

    if matches.get_flag("daemonize") {
        config.daemonize = cli_args.daemonize;
    }

    if matches.get_many::<Dir>("dirs").is_some() {
        config.dirs = cli_args.dirs;
    }

    // Set the config path for reference
    config.config = cli_args.config;

    Ok(config)
}

fn main() -> Result<(), Box<dyn Error>> {
    let binding = &GLOBAL_PARAMS;
    let mut cli = binding.lock()?;
    *cli = load_config()?;

    //    println!("Using config: {:?}", *cli);
    for dir in &cli.dirs {
        println!(
            "Watch source: \"{}\"",
            dir.source.clone().ok_or("Cannot clone")?.to_string_lossy()
        );
        println!(
            "      target: \"{}\"",
            dir.target
                .clone()
                .unwrap_or(dir.source.clone().ok_or("Cannot clone")?)
                .to_string_lossy()
        );
        println!("      create decade dir: {}", !dir.nodecade);
        println!("      create year dir:   {}", !dir.noyear);
        println!("      create month dir:  {}", !dir.nomonth);
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
            dir.source.clone().ok_or("Cannot clone")?.canonicalize()?,
            &dir.target,
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

                            process_fname(nodecade, noyear, nomonth, path.to_path_buf(), &target)
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

                            process_fname(nodecade, noyear, nomonth, path.to_path_buf(), &target)
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

// Функция для извлечения даты из EXIF метаданных
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

    // Приоритет тегов для даты создания
    let date_tags = [
        ExifTag::DateTimeOriginal, // Дата и время создания оригинального изображения
        ExifTag::CreateDate,       // Стандартная дата/время
    ];

    let mut result_date = None;
    for &tag in &date_tags {
        if let Some(field) = exif.get(tag) {
            let time = field.as_time().unwrap();
            result_date = Some((
                format!("{:04}-{:02}-{:02}", time.year(), time.month(), time.day()),
                exif.get_gps_info().unwrap(),
            ));
            break;
        }
    }

    result_date
}

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

    // Приоритет тегов для даты создания
    let date_tags = [
        TrackInfoTag::CreateDate, // Стандартная дата/время
    ];

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

// Функция для проверки валидности даты в формате YYYY-MM-DD
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

    // Проверяем валидность даты через библиотеку chrono
    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

// Функция для проверки и преобразования формата YYYYMMDD в YYYY-MM-DD
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

    // Проверяем, что дата действительно валидна
    NaiveDate::from_ymd_opt(year_num, month_num, day_num)?;

    Some(format!("{}-{}-{}", year, month, day))
}

// Функция для проверки и преобразования формата YYYY_MMDD в YYYY-MM-DD
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

    // Проверяем, что дата действительно валидна
    NaiveDate::from_ymd_opt(year_num, month_num, day_num)?;

    Some(format!("{}-{}-{}", year, month, day))
}
