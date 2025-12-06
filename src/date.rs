//! Date extraction and validation utilities for sort_pictures.
//!
//! This module extracts dates from EXIF metadata, video track metadata,
//! and filename patterns. It validates dates are within acceptable range (1990-2099).

use chrono::{Datelike, NaiveDate};
use nom_exif::{
    Exif, ExifIter, ExifTag, GPSInfo, MediaParser, MediaSource, TrackInfo, TrackInfoTag,
};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// Where the date was extracted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSource {
    /// Date from EXIF DateTimeOriginal tag.
    ExifDateTimeOriginal,
    /// Date from EXIF CreateDate tag.
    ExifCreateDate,
    /// Date from EXIF ModifyDate tag.
    ExifModifyDate,
    /// Date from video track metadata.
    TrackMetadata,
    /// Date extracted from filename pattern.
    FilenamePattern,
}

/// Result of date extraction from a file.
#[derive(Debug, Clone)]
pub struct ExtractedDate {
    /// Date string in YYYY-MM-DD format.
    pub date: String,
    /// Source of the date (for logging/debugging).
    pub source: DateSource,
    /// Optional GPS info extracted along with the date.
    pub gps: Option<GPSInfo>,
}

// Static regex patterns, compiled once
static YYYY_MM_DD_PREFIX: OnceLock<Regex> = OnceLock::new();
static YYYY_MM_DD_EMBEDDED: OnceLock<Regex> = OnceLock::new();
static YYYYMMDD: OnceLock<Regex> = OnceLock::new();
static YYYY_MMDD: OnceLock<Regex> = OnceLock::new();

/// Returns compiled regex for YYYY-MM-DD at start of filename.
fn yyyy_mm_dd_prefix_regex() -> &'static Regex {
    YYYY_MM_DD_PREFIX.get_or_init(|| Regex::new(r"^(\d{4}[-_]\d{2}[-_]\d{2})").unwrap())
}

/// Returns compiled regex for YYYY-MM-DD embedded in filename.
fn yyyy_mm_dd_embedded_regex() -> &'static Regex {
    YYYY_MM_DD_EMBEDDED.get_or_init(|| Regex::new(r"[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})").unwrap())
}

/// Returns compiled regex for YYYYMMDD format.
fn yyyymmdd_regex() -> &'static Regex {
    YYYYMMDD.get_or_init(|| Regex::new(r"(\d{8})").unwrap())
}

/// Returns compiled regex for YYYY_MMDD format.
fn yyyy_mmdd_regex() -> &'static Regex {
    YYYY_MMDD.get_or_init(|| Regex::new(r"(\d{4})_(\d{4})").unwrap())
}

/// Extract date from file using all available methods.
///
/// Priority: EXIF -> Track metadata -> Filename patterns
///
/// # Arguments
/// * `path` - Path to the file to extract date from
///
/// # Returns
/// `Some(ExtractedDate)` if a valid date was found, `None` otherwise.
pub fn extract_date(path: &Path) -> Option<ExtractedDate> {
    let mut parser = MediaParser::new();

    // Try EXIF metadata first
    if let Some(result) = extract_date_from_exif_with_parser(&mut parser, path) {
        return Some(result);
    }

    // Try track metadata for videos
    if let Some(result) = extract_date_from_track_with_parser(&mut parser, path) {
        return Some(result);
    }

    // Fall back to filename patterns
    let filename = path.file_name()?.to_string_lossy();
    extract_date_from_filename(&filename)
}

/// Extract date from EXIF metadata only.
///
/// # Arguments
/// * `path` - Path to the file to extract date from
///
/// # Returns
/// `Some(ExtractedDate)` if EXIF date was found, `None` otherwise.
pub fn extract_date_from_exif(path: &Path) -> Option<ExtractedDate> {
    let mut parser = MediaParser::new();
    extract_date_from_exif_with_parser(&mut parser, path)
}

/// Internal function to extract date from EXIF with a reusable parser.
fn extract_date_from_exif_with_parser(
    parser: &mut MediaParser,
    path: &Path,
) -> Option<ExtractedDate> {
    let ms = MediaSource::file_path(path).ok()?;
    if !ms.has_exif() {
        return None;
    }

    let iter: ExifIter = match parser.parse(ms) {
        Ok(p) => p,
        Err(e) => {
            println!("Cannot parse EXIF: {}", e);
            return None;
        }
    };
    let exif: Exif = iter.into();

    // Priority order for date tags
    let date_tags = [
        (ExifTag::DateTimeOriginal, DateSource::ExifDateTimeOriginal),
        (ExifTag::CreateDate, DateSource::ExifCreateDate),
        (ExifTag::ModifyDate, DateSource::ExifModifyDate),
    ];

    for (tag, source) in date_tags {
        if let Some(field) = exif.get(tag) {
            let time_opt = match field {
                nom_exif::EntryValue::Time(dt) => Some((dt.year(), dt.month(), dt.day())),
                nom_exif::EntryValue::NaiveDateTime(dt) => Some((dt.year(), dt.month(), dt.day())),
                _ => field.as_time().map(|dt| (dt.year(), dt.month(), dt.day())),
            };

            if let Some((year, month, day)) = time_opt {
                let date = format!("{:04}-{:02}-{:02}", year, month, day);
                if is_valid_date_str(&date) {
                    return Some(ExtractedDate {
                        date,
                        source,
                        gps: exif.get_gps_info().ok().flatten(),
                    });
                }
            }
        }
    }

    None
}

/// Internal function to extract date from video track metadata.
fn extract_date_from_track_with_parser(
    parser: &mut MediaParser,
    path: &Path,
) -> Option<ExtractedDate> {
    let ms = MediaSource::file_path(path).ok()?;
    if !ms.has_track() {
        return None;
    }

    let track: TrackInfo = match parser.parse(ms) {
        Ok(p) => p,
        Err(e) => {
            println!("Cannot parse track: {}", e);
            return None;
        }
    };

    if let Some(field) = track.get(TrackInfoTag::CreateDate)
        && let Some(time) = field.as_time()
    {
        let date = format!("{:04}-{:02}-{:02}", time.year(), time.month(), time.day());
        if is_valid_date_str(&date) {
            return Some(ExtractedDate {
                date,
                source: DateSource::TrackMetadata,
                gps: track.get_gps_info().cloned(),
            });
        }
    }

    None
}

/// Extract date from filename using regex patterns.
///
/// Patterns checked in order:
/// 1. YYYY-MM-DD or YYYY_MM_DD at start of filename
/// 2. YYYY-MM-DD or YYYY_MM_DD embedded in filename
/// 3. YYYY_MMDD format (e.g., 2020_0718)
/// 4. YYYYMMDD format anywhere in filename
///
/// # Arguments
/// * `filename` - The filename (without path) to extract date from
///
/// # Returns
/// `Some(ExtractedDate)` if a valid date pattern was found, `None` otherwise.
pub fn extract_date_from_filename(filename: &str) -> Option<ExtractedDate> {
    // Pattern 1: YYYY-MM-DD at start of filename
    if let Some(captures) = yyyy_mm_dd_prefix_regex().captures(filename) {
        let date_str = captures.get(1)?.as_str().replace('_', "-");
        if is_valid_date_str(&date_str) {
            return Some(ExtractedDate {
                date: date_str,
                source: DateSource::FilenamePattern,
                gps: None,
            });
        }
    }

    // Pattern 2: YYYY-MM-DD embedded in filename
    let padded_filename = format!(" {}", filename);
    if let Some(captures) = yyyy_mm_dd_embedded_regex().captures(&padded_filename) {
        let date_str = captures.get(1)?.as_str().replace('_', "-");
        if is_valid_date_str(&date_str) {
            return Some(ExtractedDate {
                date: date_str,
                source: DateSource::FilenamePattern,
                gps: None,
            });
        }
    }

    // Pattern 3: YYYY_MMDD format (e.g., 2020_0718_064509.MP4)
    if let Some(captures) = yyyy_mmdd_regex().captures(filename) {
        let year = captures.get(1)?.as_str();
        let mmdd = captures.get(2)?.as_str();
        if let Some(date_str) = try_parse_yyyy_mmdd(year, mmdd) {
            return Some(ExtractedDate {
                date: date_str,
                source: DateSource::FilenamePattern,
                gps: None,
            });
        }
    }

    // Pattern 4: YYYYMMDD format anywhere in filename
    if let Some(captures) = yyyymmdd_regex().captures(filename) {
        let date_part = captures.get(1)?.as_str();
        if let Some(date_str) = try_parse_yyyymmdd(date_part) {
            return Some(ExtractedDate {
                date: date_str,
                source: DateSource::FilenamePattern,
                gps: None,
            });
        }
    }

    None
}

/// Validates a date string in YYYY-MM-DD format.
///
/// # Arguments
/// * `date_str` - Date string to validate
///
/// # Returns
/// `true` if the date is valid and within range 1990-2099.
pub fn is_valid_date_str(date_str: &str) -> bool {
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

    is_valid_date(year, month, day)
}

/// Validates date components are within acceptable range.
///
/// # Arguments
/// * `year` - Year (1990-2099)
/// * `month` - Month (1-12)
/// * `day` - Day (1-31, validated by chrono)
///
/// # Returns
/// `true` if the date is valid.
pub fn is_valid_date(year: i32, month: u32, day: u32) -> bool {
    if !(1990..=2099).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }

    NaiveDate::from_ymd_opt(year, month, day).is_some()
}

/// Format date components into YYYY-MM-DD string.
pub fn format_date(year: i32, month: u32, day: u32) -> String {
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Attempts to parse YYYYMMDD format to YYYY-MM-DD.
///
/// # Arguments
/// * `date_str` - 8-character date string in YYYYMMDD format
///
/// # Returns
/// `Some(String)` with YYYY-MM-DD format if valid, `None` otherwise.
pub fn try_parse_yyyymmdd(date_str: &str) -> Option<String> {
    if date_str.len() != 8 {
        return None;
    }

    let year = &date_str[0..4];
    let month = &date_str[4..6];
    let day = &date_str[6..8];

    let year_num = year.parse::<i32>().ok()?;
    let month_num = month.parse::<u32>().ok()?;
    let day_num = day.parse::<u32>().ok()?;

    if !is_valid_date(year_num, month_num, day_num) {
        return None;
    }

    Some(format!("{}-{}-{}", year, month, day))
}

/// Attempts to parse YYYY_MMDD format to YYYY-MM-DD.
///
/// # Arguments
/// * `year` - 4-character year string
/// * `mmdd` - 4-character month/day string
///
/// # Returns
/// `Some(String)` with YYYY-MM-DD format if valid, `None` otherwise.
pub fn try_parse_yyyy_mmdd(year: &str, mmdd: &str) -> Option<String> {
    if mmdd.len() != 4 {
        return None;
    }

    let month = &mmdd[0..2];
    let day = &mmdd[2..4];

    let year_num = year.parse::<i32>().ok()?;
    let month_num = month.parse::<u32>().ok()?;
    let day_num = day.parse::<u32>().ok()?;

    if !is_valid_date(year_num, month_num, day_num) {
        return None;
    }

    Some(format!("{}-{}-{}", year, month, day))
}

/// Calculate decade folder name (e.g., "2020-2029").
pub fn decade_folder(year: i32) -> String {
    let decade_start = (year / 10) * 10;
    let decade_end = decade_start + 9;
    format!("{}-{}", decade_start, decade_end)
}

/// Calculate month folder name (e.g., "03").
pub fn month_folder(month: u32) -> String {
    format!("{:02}", month)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Date validation tests
    #[test]
    fn test_is_valid_date_valid() {
        assert!(is_valid_date(2024, 3, 15));
        assert!(is_valid_date(1990, 1, 1));
        assert!(is_valid_date(2099, 12, 31));
    }

    #[test]
    fn test_is_valid_date_invalid_year() {
        assert!(!is_valid_date(1989, 3, 15)); // Too old
        assert!(!is_valid_date(2100, 3, 15)); // Too new
    }

    #[test]
    fn test_is_valid_date_invalid_month() {
        assert!(!is_valid_date(2024, 0, 15));
        assert!(!is_valid_date(2024, 13, 15));
    }

    #[test]
    fn test_is_valid_date_invalid_day() {
        assert!(!is_valid_date(2024, 3, 0));
        assert!(!is_valid_date(2024, 3, 32));
        assert!(!is_valid_date(2024, 2, 30)); // Feb 30 doesn't exist
    }

    #[test]
    fn test_is_valid_date_leap_year() {
        assert!(is_valid_date(2024, 2, 29)); // 2024 is a leap year
        assert!(!is_valid_date(2023, 2, 29)); // 2023 is not
    }

    // Date string validation tests
    #[test]
    fn test_is_valid_date_str_valid() {
        assert!(is_valid_date_str("2024-03-15"));
        assert!(is_valid_date_str("1990-01-01"));
    }

    #[test]
    fn test_is_valid_date_str_invalid_format() {
        assert!(!is_valid_date_str("2024/03/15")); // Wrong separator
        assert!(!is_valid_date_str("24-03-15")); // Short year
        assert!(!is_valid_date_str("2024-3-15")); // Short month
    }

    // YYYYMMDD parsing tests
    #[test]
    fn test_try_parse_yyyymmdd_valid() {
        assert_eq!(
            try_parse_yyyymmdd("20240315"),
            Some("2024-03-15".to_string())
        );
        assert_eq!(
            try_parse_yyyymmdd("19900101"),
            Some("1990-01-01".to_string())
        );
    }

    #[test]
    fn test_try_parse_yyyymmdd_invalid() {
        assert_eq!(try_parse_yyyymmdd("20241315"), None); // Invalid month
        assert_eq!(try_parse_yyyymmdd("20240230"), None); // Feb 30
        assert_eq!(try_parse_yyyymmdd("2024031"), None); // Too short
        assert_eq!(try_parse_yyyymmdd("202403150"), None); // Too long
    }

    // YYYY_MMDD parsing tests
    #[test]
    fn test_try_parse_yyyy_mmdd_valid() {
        assert_eq!(
            try_parse_yyyy_mmdd("2024", "0315"),
            Some("2024-03-15".to_string())
        );
        assert_eq!(
            try_parse_yyyy_mmdd("2020", "0718"),
            Some("2020-07-18".to_string())
        );
    }

    #[test]
    fn test_try_parse_yyyy_mmdd_invalid() {
        assert_eq!(try_parse_yyyy_mmdd("2024", "1315"), None); // Invalid month
        assert_eq!(try_parse_yyyy_mmdd("2024", "031"), None); // Too short
    }

    // Filename pattern tests
    #[test]
    fn test_extract_date_from_filename_prefix() {
        let result = extract_date_from_filename("2024-03-15_photo.jpg");
        assert!(result.is_some());
        let extracted = result.unwrap();
        assert_eq!(extracted.date, "2024-03-15");
        assert_eq!(extracted.source, DateSource::FilenamePattern);
    }

    #[test]
    fn test_extract_date_from_filename_prefix_underscore() {
        let result = extract_date_from_filename("2024_03_15_photo.jpg");
        assert!(result.is_some());
        assert_eq!(result.unwrap().date, "2024-03-15");
    }

    #[test]
    fn test_extract_date_from_filename_embedded() {
        // Note: The embedded regex requires a non-digit, non-hyphen char before the date
        // So "IMG_2024-03-15_photo.jpg" works ("_" before date)
        let result = extract_date_from_filename("IMG_2024-03-15_photo.jpg");
        assert!(result.is_some());
        assert_eq!(result.unwrap().date, "2024-03-15");
    }

    #[test]
    fn test_extract_date_from_filename_yyyymmdd() {
        let result = extract_date_from_filename("IMG_20240315_120000.jpg");
        assert!(result.is_some());
        assert_eq!(result.unwrap().date, "2024-03-15");
    }

    #[test]
    fn test_extract_date_from_filename_yyyy_mmdd() {
        let result = extract_date_from_filename("2020_0718_064509.MP4");
        assert!(result.is_some());
        assert_eq!(result.unwrap().date, "2020-07-18");
    }

    #[test]
    fn test_extract_date_from_filename_no_match() {
        assert!(extract_date_from_filename("photo.jpg").is_none());
        assert!(extract_date_from_filename("random_name.png").is_none());
    }

    // Folder name tests
    #[test]
    fn test_decade_folder() {
        assert_eq!(decade_folder(2024), "2020-2029");
        assert_eq!(decade_folder(2020), "2020-2029");
        assert_eq!(decade_folder(2029), "2020-2029");
        assert_eq!(decade_folder(1995), "1990-1999");
    }

    #[test]
    fn test_month_folder() {
        assert_eq!(month_folder(1), "01");
        assert_eq!(month_folder(3), "03");
        assert_eq!(month_folder(12), "12");
    }

    // Format date test
    #[test]
    fn test_format_date() {
        assert_eq!(format_date(2024, 3, 15), "2024-03-15");
        assert_eq!(format_date(1990, 1, 1), "1990-01-01");
    }
}
