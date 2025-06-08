use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use regex::Regex;
use chrono::NaiveDate;
use std::ffi::OsStr;
use exif::{In, Tag, Value};
use clap::Parser;
use std::sync::Mutex;

#[derive(Parser)]
#[command(name = "sort_pictures")]
#[command(about = "A program to re-order pictures in the directory")]
struct Cli {
    /// Disable creation of "decade" directory.
    #[arg(long)]
    nodecade: bool,
    /// Disable creation of "year" directory.
    #[arg(long)]
    noyear: bool,
    /// Disable creation of "month" directory.
    #[arg(long)]
    nomonth: bool,
}

impl Cli {
    const fn empty() -> Self {
	Self {
	    nodecade: false,
	    noyear: false,
	    nomonth: false,
	}
    }
}

const GLOBAL_PARAMS:Mutex<Cli> = Mutex::new(Cli::empty());

fn main() -> std::io::Result<()> {
    let binding = GLOBAL_PARAMS;
    let mut cli = binding.lock().unwrap();
    *cli = Cli::parse();
    
    // Создаем регулярные выражения для различных форматов дат
    let yyyy_mm_dd_prefix_regex = Regex::new(r"^(\d{4}[-_]\d{2}[-_]\d{2})").unwrap();
    let yyyy_mm_dd_embedded_regex = Regex::new(r"[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})").unwrap();
    let yyyymmdd_regex = Regex::new(r"(\d{8})").unwrap();
    let yyyy_mmdd_regex = Regex::new(r"(\d{4})_(\d{4})").unwrap();
    
    // Получаем текущую директорию
    let current_dir = std::env::current_dir()?;
    
    // Множество для хранения уникальных дат в формате YYYY-MM-DD
    let mut date_dirs: HashSet<String> = HashSet::new();
    
    // Словарь для хранения информации о файлах и их датах
    let mut file_date_map: Vec<(PathBuf, String)> = Vec::new();
    
    // Первый проход: собираем информацию о файлах и датах
    let entries = fs::read_dir(&current_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        // Пропускаем директории и скрытые файлы
        if path.is_dir() || path.file_name().unwrap().to_string_lossy().starts_with(".") 
            || path.extension().map_or(false, |ext| ext == "sh" || ext == "rs") {
            continue;
        }
        
        let filename = path.file_name().unwrap().to_string_lossy();
        let mut date_found = false;
        
        // Проверяем, является ли файл изображением, и пытаемся прочитать EXIF
        let extension = path.extension().and_then(OsStr::to_str).unwrap_or("").to_lowercase();
        if is_image_file(&extension) {
            if let Some(exif_date) = extract_date_from_exif(&path) {
                let exif_date_clone = exif_date.clone();
                date_dirs.insert(exif_date.clone());
                file_date_map.push((path.to_owned(), exif_date));
                date_found = true;
                println!("Найдена EXIF-дата для файла {}: {}", filename, exif_date_clone);
            }
        }
        
        // Если EXIF не сработал, применяем анализ имени файла
        if !date_found {
            // Проверяем формат YYYY-MM-DD в начале файла
            if let Some(captures) = yyyy_mm_dd_prefix_regex.captures(&filename) {
                let date_str = captures.get(1).unwrap().as_str().to_string().replace("_","-");
                if is_valid_date(&date_str) {
                    date_dirs.insert(date_str.clone());
                    file_date_map.push((path.to_owned(), date_str));
                    date_found = true;
                }
            }
        }
        
        // Проверяем формат YYYY-MM-DD в середине строки
        if !date_found {
            let padded_filename = format!(" {}", filename);
            if let Some(captures) = yyyy_mm_dd_embedded_regex.captures(&padded_filename) {
                let date_str = captures.get(1).unwrap().as_str().to_string().replace("_","-");
                if is_valid_date(&date_str) {
                    date_dirs.insert(date_str.clone());
                    file_date_map.push((path.to_owned(), date_str));
                    date_found = true;
                }
            }
        }
        
        // Проверяем формат YYYY_MMDD (как в 2020_0718_064509_034.MP4)
        if !date_found {
            if let Some(captures) = yyyy_mmdd_regex.captures(&filename) {
                let year = captures.get(1).unwrap().as_str();
                let mmdd = captures.get(2).unwrap().as_str();
                
                if let Some(date_str) = try_parse_yyyy_mmdd(year, mmdd) {
                    date_dirs.insert(date_str.clone());
                    file_date_map.push((path.to_owned(), date_str));
                    date_found = true;
                }
            }
        }
        
        // Проверяем формат YYYYMMDD где-либо в имени файла
        if !date_found {
            if let Some(captures) = yyyymmdd_regex.captures(&filename) {
                let date_part = captures.get(1).unwrap().as_str();
                if let Some(date_str) = try_parse_yyyymmdd(date_part) {
                    date_dirs.insert(date_str.clone());
                    file_date_map.push((path.to_owned(), date_str));
                    // date_found = true; -- не используется в дальнейшем
                }
            }
        }
    }
    
    // Выводим найденные даты
    let mut sorted_dates: Vec<&String> = date_dirs.iter().collect();
    sorted_dates.sort();
    
    if sorted_dates.is_empty() {
        println!("Не найдено файлов с датами");
        return Ok(());
    }
    
    println!("Найдены следующие даты:");
    for date in &sorted_dates {
        println!("- {}", date);
    }
    
    // Создаем директории и перемещаем файлы
    for date in sorted_dates {
        println!("Обрабатываю дату: {}", date);

	let parts: Vec<&str> = date.split('-').collect();
	let year_str = parts[0];
        let month = parts[1];
        
        // Преобразуем строку в число
        let year = year_str.parse::<i32>().unwrap();
    
        // Вычисляем начало и конец десятилетия
        let decade_start = (year / 10) * 10; // Округляем до начала десятилетия
        let decade_end = decade_start + 9;
    
        // Форматируем результат
        let decade_range = &format!("{}-{}", decade_start, decade_end);

	// Создаем директорию для даты, если она еще не существует
        let mut date_dir = current_dir.clone();

	if !cli.nodecade {
	    date_dir = date_dir.join(decade_range);
	}

	if !cli.noyear {
	    date_dir = date_dir.join(year_str);
	}

	if !cli.nomonth {
	    date_dir = date_dir.join(month);
	}

	date_dir = date_dir.join(date);
        if !date_dir.exists() {
            fs::create_dir_all(&date_dir)?;
        }
        
        // Перемещаем файлы, соответствующие текущей дате
        for (file_path, file_date) in &file_date_map {
            if file_date == date && file_path.exists() {
                let filename = file_path.file_name().unwrap();
                let target_path = date_dir.join(filename);
                
                print!("  Перемещение файла: {}... ", filename.to_string_lossy());
                io::stdout().flush()?;
                
                match fs::rename(file_path, &target_path) {
                    Ok(_) => println!("успешно"),
                    Err(e) => println!("ошибка: {}", e),
                }
            }
        }
        
        println!("Файлы с датой {} перемещены в директорию {}/{}/{}/{}", date, decade_range, year, month, date);
    }
    
    println!("Завершено!");
    Ok(())
}

// Функция для определения, является ли файл изображением
fn is_image_file(extension: &str) -> bool {
    matches!(extension, "jpg" | "jpeg" | "png" | "gif" | "tiff" | "bmp" | "webp" | "heic" | "heif")
}

// Функция для извлечения даты из EXIF метаданных
fn extract_date_from_exif(path: &Path) -> Option<String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return None,
    };
    
    let mut bufreader = std::io::BufReader::new(&file);
    let exifreader = match exif::Reader::new().read_from_container(&mut bufreader) {
        Ok(exif) => exif,
        Err(_) => return None,
    };
    
    // Приоритет тегов для даты создания
    let date_tags = [
        Tag::DateTimeOriginal,  // Дата и время создания оригинального изображения
        Tag::DateTime,          // Стандартная дата/время
    ];
    
    for &tag in &date_tags {
        if let Some(field) = exifreader.get_field(tag, In::PRIMARY) {
            if let Some(date_str) = parse_exif_date(&field.value) {
                return Some(date_str);
            }
        }
    }
    
    None
}

// Функция для разбора даты из EXIF значения
fn parse_exif_date(value: &Value) -> Option<String> {
    if let Value::Ascii(vec) = value {
        if !vec.is_empty() {
            // EXIF дата обычно в формате "YYYY:MM:DD HH:MM:SS"
            let date_str = String::from_utf8_lossy(&vec[0]);
            
            // Преобразуем в нужный нам формат YYYY-MM-DD
            if let Some(date_part) = date_str.split_whitespace().next() {
                let parts: Vec<&str> = date_part.split(':').collect();
                if parts.len() >= 3 {
                    let year = parts[0];
                    let month = parts[1];
                    let day = parts[2];
                    
                    // Проверяем валидность даты
                    if let (Ok(y), Ok(m), Ok(d)) = (year.parse::<i32>(), month.parse::<u32>(), day.parse::<u32>()) {
                        if y >= 1990 && y <= 2099 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                            if NaiveDate::from_ymd_opt(y, m, d).is_some() {
                                return Some(format!("{}-{}-{}", year, month, day));
                            }
                        }
                    }
                }
            }
        }
    }
    
    None
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
    
    if year < 1990 || year > 2099 || month < 1 || month > 12 || day < 1 || day > 31 {
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
    
    if year_num < 1990 || year_num > 2099 || month_num < 1 || month_num > 12 || day_num < 1 || day_num > 31 {
        return None;
    }
    
    // Проверяем, что дата действительно валидна
    if NaiveDate::from_ymd_opt(year_num, month_num, day_num).is_none() {
        return None;
    }
    
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
    
    if year_num < 1990 || year_num > 2099 || month_num < 1 || month_num > 12 || day_num < 1 || day_num > 31 {
        return None;
    }
    
    // Проверяем, что дата действительно валидна
    if NaiveDate::from_ymd_opt(year_num, month_num, day_num).is_none() {
        return None;
    }
    
    Some(format!("{}-{}-{}", year, month, day))
}
