//! File system operations for sort_pictures.
//!
//! Handles directory scanning, file filtering, path construction,
//! and safe file moving with collision detection.

use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::{Path, PathBuf};

use crate::date;

/// Reasons why a file might be skipped during processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// Path is a directory, not a file.
    IsDirectory,
    /// File starts with '.' (hidden file).
    HiddenFile,
    /// File has a skipped extension (.sh, .rs).
    SkippedExtension(String),
}

/// Result of processing a single file.
#[derive(Debug)]
pub enum ProcessResult {
    /// File was moved successfully.
    Moved {
        /// Original file path.
        from: PathBuf,
        /// New file path after move.
        to: PathBuf,
    },
    /// File was skipped (with reason).
    Skipped {
        /// Path of skipped file.
        path: PathBuf,
        /// Why it was skipped.
        reason: SkipReason,
    },
}

/// Extensions that should always be skipped.
const SKIPPED_EXTENSIONS: &[&str] = &["sh", "rs"];

/// Check if a file should be skipped during processing.
///
/// Files are skipped if they are:
/// - Directories
/// - Hidden files (start with '.')
/// - Script files (.sh, .rs)
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// `Some(SkipReason)` if the file should be skipped, `None` if it should be processed.
pub fn should_skip_file(path: &Path) -> Option<SkipReason> {
    // Skip directories
    if path.is_dir() {
        return Some(SkipReason::IsDirectory);
    }

    // Skip hidden files
    if let Some(name) = path.file_name()
        && name.to_string_lossy().starts_with('.')
    {
        return Some(SkipReason::HiddenFile);
    }

    // Skip certain extensions
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if SKIPPED_EXTENSIONS.contains(&ext_str.as_str()) {
            return Some(SkipReason::SkippedExtension(ext_str));
        }
    }

    None
}

/// Get all processable files from a directory (non-recursive).
///
/// Returns only files that pass the `should_skip_file` check.
///
/// # Arguments
/// * `dir` - Directory to scan
///
/// # Returns
/// Vector of paths to processable files.
pub fn scan_directory(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir)?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if should_skip_file(&path).is_none() {
            files.push(path);
        }
    }

    Ok(files)
}

/// Build target path with date hierarchy.
///
/// Constructs the full target directory path based on the date and hierarchy settings.
///
/// # Arguments
/// * `base_target` - Base target directory
/// * `date` - Date string in YYYY-MM-DD format
/// * `nodecade` - If true, skip decade folder
/// * `noyear` - If true, skip year folder
/// * `nomonth` - If true, skip month folder
///
/// # Returns
/// Full target directory path including date folders.
pub fn build_target_path(
    base_target: &Path,
    date: &str,
    nodecade: bool,
    noyear: bool,
    nomonth: bool,
) -> PathBuf {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        // Invalid date format, just return base with date
        return base_target.join(date);
    }

    let year_str = parts[0];
    let month = parts[1];
    let year: i32 = year_str.parse().unwrap_or(2000);

    let mut path = base_target.to_path_buf();

    if !nodecade {
        path = path.join(date::decade_folder(year));
    }

    if !noyear {
        path = path.join(year_str);
    }

    if !nomonth {
        path = path.join(month);
    }

    // Always add the date folder
    path = path.join(date);

    path
}

/// Generate unique filename if collision exists.
///
/// Appends -1, -2, etc. before extension until a unique name is found.
///
/// # Arguments
/// * `target_dir` - Directory where file will be placed
/// * `filename` - Original filename
///
/// # Returns
/// Unique filename (original if no collision, or with -N suffix).
pub fn resolve_collision(target_dir: &Path, filename: &str) -> String {
    let original_path = target_dir.join(filename);
    if !original_path.exists() {
        return filename.to_string();
    }

    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let new_name = format!("{}-{}{}", stem, counter, extension);
        let new_path = target_dir.join(&new_name);
        if !new_path.exists() {
            return new_name;
        }
        counter += 1;
    }
}

/// Move file to target, handling collisions with -N suffix.
///
/// Creates target directories if needed. Includes a 1-second delay
/// before moving to allow filesystem sync for newly created files.
///
/// # Arguments
/// * `source` - Source file path
/// * `target_dir` - Target directory
/// * `filename` - Desired filename (collision handling will be applied)
///
/// # Returns
/// The final path where the file was moved.
pub fn move_file_safe(source: &Path, target_dir: &Path, filename: &str) -> io::Result<PathBuf> {
    // Create target directory if needed
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }

    // Resolve any filename collisions
    let final_filename = resolve_collision(target_dir, filename);
    let target_path = target_dir.join(&final_filename);

    // Small delay for filesystem sync
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Log the move operation
    let (base, rel_source, rel_target) = crate::path::find_common_base(source, &target_path);
    print!(
        "Move: \"{}\"/{{\"{}\" -> \"{}\"}}... ",
        base.to_string_lossy(),
        rel_source.to_string_lossy(),
        rel_target.to_string_lossy()
    );
    io::stdout().flush()?;

    // Perform the move
    match fs::rename(source, &target_path) {
        Ok(_) => {
            println!("success");
            Ok(target_path)
        }
        Err(e) => {
            println!("error: {}", e);
            Err(e)
        }
    }
}

/// Move file without logging output.
///
/// Same as `move_file_safe` but without console output.
/// Useful for testing or batch operations.
pub fn move_file_quiet(source: &Path, target_dir: &Path, filename: &str) -> io::Result<PathBuf> {
    // Create target directory if needed
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }

    // Resolve any filename collisions
    let final_filename = resolve_collision(target_dir, filename);
    let target_path = target_dir.join(&final_filename);

    // Perform the move
    fs::rename(source, &target_path)?;
    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Atomic counter for unique test directory names
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    // Helper to create a unique temp directory per test
    fn temp_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "sort_pictures_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Cleanup helper
    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // should_skip_file tests
    #[test]
    fn test_should_skip_hidden_file() {
        assert_eq!(
            should_skip_file(Path::new(".hidden")),
            Some(SkipReason::HiddenFile)
        );
        assert_eq!(
            should_skip_file(Path::new("/path/to/.gitignore")),
            Some(SkipReason::HiddenFile)
        );
    }

    #[test]
    fn test_should_skip_script_files() {
        assert_eq!(
            should_skip_file(Path::new("script.sh")),
            Some(SkipReason::SkippedExtension("sh".to_string()))
        );
        assert_eq!(
            should_skip_file(Path::new("test.rs")),
            Some(SkipReason::SkippedExtension("rs".to_string()))
        );
    }

    #[test]
    fn test_should_not_skip_normal_files() {
        assert_eq!(should_skip_file(Path::new("photo.jpg")), None);
        assert_eq!(should_skip_file(Path::new("video.mp4")), None);
        assert_eq!(should_skip_file(Path::new("image.png")), None);
    }

    // build_target_path tests
    #[test]
    fn test_build_target_path_full_hierarchy() {
        let path = build_target_path(Path::new("/photos"), "2024-03-15", false, false, false);
        assert_eq!(path, Path::new("/photos/2020-2029/2024/03/2024-03-15"));
    }

    #[test]
    fn test_build_target_path_no_decade() {
        let path = build_target_path(Path::new("/photos"), "2024-03-15", true, false, false);
        assert_eq!(path, Path::new("/photos/2024/03/2024-03-15"));
    }

    #[test]
    fn test_build_target_path_no_year() {
        let path = build_target_path(Path::new("/photos"), "2024-03-15", false, true, false);
        assert_eq!(path, Path::new("/photos/2020-2029/03/2024-03-15"));
    }

    #[test]
    fn test_build_target_path_no_month() {
        let path = build_target_path(Path::new("/photos"), "2024-03-15", false, false, true);
        assert_eq!(path, Path::new("/photos/2020-2029/2024/2024-03-15"));
    }

    #[test]
    fn test_build_target_path_no_hierarchy() {
        let path = build_target_path(Path::new("/photos"), "2024-03-15", true, true, true);
        assert_eq!(path, Path::new("/photos/2024-03-15"));
    }

    // resolve_collision tests
    #[test]
    fn test_resolve_collision_no_conflict() {
        let temp = temp_dir();
        let result = resolve_collision(&temp, "photo.jpg");
        assert_eq!(result, "photo.jpg");
        cleanup(&temp);
    }

    #[test]
    fn test_resolve_collision_single_conflict() {
        let temp = temp_dir();
        File::create(temp.join("photo.jpg")).unwrap();

        let result = resolve_collision(&temp, "photo.jpg");
        assert_eq!(result, "photo-1.jpg");
        cleanup(&temp);
    }

    #[test]
    fn test_resolve_collision_multiple_conflicts() {
        let temp = temp_dir();
        File::create(temp.join("photo.jpg")).unwrap();
        File::create(temp.join("photo-1.jpg")).unwrap();
        File::create(temp.join("photo-2.jpg")).unwrap();

        let result = resolve_collision(&temp, "photo.jpg");
        assert_eq!(result, "photo-3.jpg");
        cleanup(&temp);
    }

    #[test]
    fn test_resolve_collision_no_extension() {
        let temp = temp_dir();
        File::create(temp.join("README")).unwrap();

        let result = resolve_collision(&temp, "README");
        assert_eq!(result, "README-1");
        cleanup(&temp);
    }

    // scan_directory tests
    #[test]
    fn test_scan_directory_filters_correctly() {
        let temp = temp_dir();

        // Create test files
        File::create(temp.join("photo.jpg")).unwrap();
        File::create(temp.join(".hidden")).unwrap();
        File::create(temp.join("script.sh")).unwrap();
        File::create(temp.join("video.mp4")).unwrap();
        fs::create_dir(temp.join("subdir")).unwrap();

        let files = scan_directory(&temp).unwrap();

        // Should only contain photo.jpg and video.mp4
        assert_eq!(files.len(), 2);

        let filenames: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(filenames.contains(&"photo.jpg".to_string()));
        assert!(filenames.contains(&"video.mp4".to_string()));

        cleanup(&temp);
    }

    // move_file_quiet tests (without delay)
    #[test]
    fn test_move_file_quiet_creates_dirs() {
        let temp = temp_dir();
        let source = temp.join("source.jpg");
        fs::write(&source, b"test content").unwrap();

        let target_dir = temp.join("deep/nested/path");
        let result = move_file_quiet(&source, &target_dir, "source.jpg").unwrap();

        assert!(result.exists());
        assert!(!source.exists());
        assert_eq!(result, target_dir.join("source.jpg"));

        cleanup(&temp);
    }

    #[test]
    fn test_move_file_quiet_handles_collision() {
        let temp = temp_dir();
        let source = temp.join("source.jpg");
        fs::write(&source, b"test content").unwrap();

        let target_dir = temp.join("target");
        fs::create_dir_all(&target_dir).unwrap();
        File::create(target_dir.join("source.jpg")).unwrap();

        let result = move_file_quiet(&source, &target_dir, "source.jpg").unwrap();

        assert!(result.exists());
        assert!(!source.exists());
        assert_eq!(result, target_dir.join("source-1.jpg"));

        cleanup(&temp);
    }
}
