# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## 1. Project Overview

**sort_pictures** is a Rust daemon that automatically organizes photos and videos by date. It extracts dates from EXIF metadata, video track information, or filename patterns, then moves files into a hierarchical directory structure (`decade/year/month/date`).

### Primary Use Cases
- Automatically sorting photos from phone camera imports
- Organizing screenshots and screen recordings by date
- Routing photos from specific GPS locations to dedicated folders (e.g., gym photos)
- Batch processing existing photo collections

### Key Features
- **Multi-source date extraction**: EXIF metadata → Track metadata → Filename patterns
- **GPS-based routing**: Route photos taken at specific locations to designated folders
- **Configurable hierarchy**: Enable/disable decade, year, or month folders per source
- **Daemon mode**: Continuous file monitoring with systemd integration
- **Non-destructive**: Files are moved, never deleted; collision handling with `-N` suffix

### Technical Requirements
- **Rust Edition**: 2024
- **Minimum Rust Version**: Requires nightly or stable with edition 2024 support
- **Target Platform**: Linux (systemd user service), but code is cross-platform

---

## 2. Repository Structure

```
sort_pictures/
├── Cargo.toml                    # Package manifest and dependencies
├── CLAUDE.md                     # This file - AI assistant guidance
├── LICENSE.txt                   # Project license
├── install.sh                    # Systemd service installation script
├── src/
│   ├── main.rs                   # Entry point, orchestration logic
│   ├── config.rs                 # Configuration structs, CLI args, config loading
│   ├── daemon.rs                 # Daemon mode, file watching, thread management
│   ├── date.rs                   # Date extraction from EXIF, tracks, and filenames
│   ├── files.rs                  # File operations, path building, collision handling
│   ├── gps.rs                    # GPS coordinate extraction and distance calculations
│   └── path.rs                   # Path utilities (find_common_base)
└── systemd/
    ├── config.toml               # Example configuration file
    └── sort_pictures.service     # Systemd unit file template
```

### Source File Responsibilities

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | ~220 | Entry point, orchestration, wires modules together |
| `src/config.rs` | ~152 | Configuration structs (`Config`, `Dir`, `Place`), CLI parsing, config file loading |
| `src/daemon.rs` | ~165 | File watching with `notify`, thread spawning per directory, event handling |
| `src/date.rs` | ~510 | EXIF/track date extraction, filename regex patterns, date validation |
| `src/files.rs` | ~450 | Directory scanning, file filtering, target path building, collision handling |
| `src/gps.rs` | ~180 | GPS coordinate conversion, geodesic distance calculation, place matching |
| `src/path.rs` | ~163 | Single utility function `find_common_base()` for clean log output formatting |

---

## 3. Build and Development Commands

### Building

```bash
# Debug build (faster compilation, includes debug symbols)
cargo build

# Release build (optimized, for deployment)
cargo build --release

# Check for compilation errors without building
cargo check
```

### Running

```bash
# Run directly (debug mode) - processes once and exits
cargo run

# Run with custom config
cargo run -- --config /path/to/config.toml

# Run as daemon (continuous monitoring)
cargo run -- --daemonize

# Run release build as daemon
./target/release/sort_pictures --daemonize
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output visible
cargo test -- --nocapture

# Run specific test by name
cargo test test_common_parent_directory

# Run tests in a specific module
cargo test path::tests

# Run tests with verbose output
cargo test -- --nocapture --test-threads=1
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check

# Run linter
cargo clippy

# Run linter with all warnings as errors
cargo clippy -- -D warnings

# Generate documentation
cargo doc --open
```

### Installation

```bash
# Install as systemd user service (runs install.sh)
./install.sh
```

---

## 4. Configuration Deep Dive

### Configuration File Location

Default: `~/.config/sort_pictures/config.toml`

Override with: `--config /path/to/config.toml`

### Full Schema

```toml
# Directory monitoring configuration
[[dirs]]
source = "/path/to/source"      # Required: Directory to monitor for new files
target = "/path/to/target"      # Optional: Where to move files (defaults to source)
nodecade = false                # Optional: Skip decade folder (e.g., 2020-2029/)
noyear = false                  # Optional: Skip year folder (e.g., 2024/)
nomonth = false                 # Optional: Skip month folder (e.g., 03/)

# GPS-based place routing
[[places]]
name = "Place Name"             # Required: Human-readable identifier for logs
target = "/path/to/target"      # Required: Destination for photos from this location
lat = 51.027290                 # Required: Latitude in decimal degrees (WGS84)
lon = 13.773699                 # Required: Longitude in decimal degrees (WGS84)
radius = 0.3                    # Required: Matching radius in kilometers
nodecade = false                # Optional: Skip decade folder
noyear = false                  # Optional: Skip year folder
nomonth = false                 # Optional: Skip month folder
```

### Default Values

| Option | Default | Description |
|--------|---------|-------------|
| `target` | Same as `source` | Target directory for organized files |
| `nodecade` | `false` | Include decade folders (2020-2029/) |
| `noyear` | `false` | Include year folders (2024/) |
| `nomonth` | `false` | Include month folders (03/) |
| `radius` | `0.0` | Place matching radius (must be > 0 to match) |

### Resulting Directory Structure

With all hierarchy enabled (default):
```
target/
└── 2020-2029/           # Decade (disabled with nodecade=true)
    └── 2024/            # Year (disabled with noyear=true)
        └── 03/          # Month (disabled with nomonth=true)
            └── 2024-03-15/  # Date (always created)
                └── photo.jpg
```

With all hierarchy disabled (`nodecade=true`, `noyear=true`, `nomonth=true`):
```
target/
└── 2024-03-15/
    └── photo.jpg
```

### Example Configurations

**Minimal - Single directory, default hierarchy:**
```toml
[[dirs]]
source = "/home/user/Pictures/Imports"
```

**Typical - Multiple sources with different targets:**
```toml
[[dirs]]
source = "/home/user/Phone/DCIM/Camera"
target = "/home/user/Pictures/Photos"

[[dirs]]
source = "/home/user/Phone/DCIM/Screenshots"
target = "/home/user/Pictures/Screenshots"
```

**Advanced - GPS-based routing:**
```toml
[[dirs]]
source = "/home/user/Phone/DCIM/Camera"
target = "/home/user/Pictures/Photos"

[[places]]
name = "Home Gym"
lat = 51.027290
lon = 13.773699
radius = 0.1
target = "/home/user/Pictures/Fitness"
nodecade = true
noyear = true

[[places]]
name = "Climbing Gym"
lat = 51.015410
lon = 13.805218
radius = 0.3
target = "/home/user/Pictures/Climbing"
```

### CLI Arguments

| Argument | Description |
|----------|-------------|
| `-c, --config <PATH>` | Path to config file (overrides default location) |
| `--daemonize` | Run as daemon with file watching |

CLI arguments override config file values when explicitly provided.

---

## 5. Architecture and Code Organization

### 5.1 Module Dependency Graph

```
main.rs (orchestration)
    │
    ├── config.rs (Config, Dir, Place, load_config)
    │   ├── clap (CLI argument parsing)
    │   ├── serde/toml (config file deserialization)
    │   └── dirs (config path resolution)
    │
    ├── date.rs (date extraction and validation)
    │   ├── nom-exif (EXIF/Track parsing)
    │   ├── regex (filename patterns, compiled once via OnceLock)
    │   └── chrono (date validation)
    │
    ├── files.rs (file operations)
    │   ├── std::fs (read_dir, rename, create_dir_all)
    │   ├── date.rs (decade_folder for path building)
    │   └── path.rs (find_common_base for logging)
    │
    ├── daemon.rs (daemon mode)
    │   ├── notify (file watching)
    │   ├── std::thread/mpsc (threading)
    │   ├── config.rs (Dir struct)
    │   └── files.rs (should_skip_file)
    │
    ├── gps.rs (GPS processing)
    │   ├── geo (geodesic distance calculations)
    │   ├── nom-exif (GPSInfo, LatLng, URational types)
    │   └── config.rs (Place struct for matching)
    │
    └── path.rs (no internal dependencies)
        └── find_common_base() (logging utility)
```

**Module hierarchy** (lower modules have no dependencies on higher ones):
```
Level 0: config.rs, path.rs (no internal deps)
Level 1: date.rs, gps.rs (depend on config)
Level 2: files.rs (depends on date, path)
Level 3: daemon.rs (depends on config, files)
Level 4: main.rs (depends on all modules)
```

### 5.2 Core Data Structures

**Location**: `src/config.rs`

```rust
/// Configuration loaded from CLI args and config file
pub struct Config {
    pub config: Option<PathBuf>,    // Config file path (CLI only, not serialized)
    pub daemonize: bool,            // Run as daemon (CLI only)
    pub dirs: Vec<Dir>,             // Directory configurations
    pub places: Vec<Place>,         // GPS-based place configurations
}

/// Directory monitoring configuration
pub struct Dir {
    pub source: Option<PathBuf>,    // Source directory to monitor
    pub target: Option<PathBuf>,    // Target directory (defaults to source)
    pub nodecade: bool,             // Disable decade folder creation
    pub noyear: bool,               // Disable year folder creation
    pub nomonth: bool,              // Disable month folder creation
}

/// GPS-based routing configuration
pub struct Place {
    pub name: String,               // Human-readable place name
    pub target: Option<PathBuf>,    // Target directory for matching photos
    pub lat: f64,                   // Latitude in decimal degrees
    pub lon: f64,                   // Longitude in decimal degrees
    pub radius: f64,                // Matching radius in kilometers
    pub nodecade: bool,             // Disable decade folder creation
    pub noyear: bool,               // Disable year folder creation
    pub nomonth: bool,              // Disable month folder creation
}
```

**Location**: `src/date.rs`

```rust
/// Result of date extraction from a file
pub struct ExtractedDate {
    pub date: String,               // Date string in YYYY-MM-DD format
    pub source: DateSource,         // Where the date came from
    pub gps: Option<GPSInfo>,       // GPS info extracted along with date
}

/// Where the date was extracted from
pub enum DateSource {
    ExifDateTimeOriginal,           // EXIF DateTimeOriginal tag
    ExifCreateDate,                 // EXIF CreateDate tag
    ExifModifyDate,                 // EXIF ModifyDate tag
    TrackMetadata,                  // Video track metadata
    FilenamePattern,                // Extracted from filename
}
```

**Location**: `src/files.rs`

```rust
/// Reasons why a file might be skipped
pub enum SkipReason {
    IsDirectory,                    // Path is a directory
    HiddenFile,                     // File starts with '.'
    SkippedExtension(String),       // Extension is .sh or .rs
}
```

**Location**: `src/daemon.rs`

```rust
/// Configuration for daemon behavior
pub struct DaemonConfig {
    pub log_skipped: bool,          // Log when files are skipped
}

/// Handle for controlling a directory watcher thread
pub struct WatchHandle {
    thread: JoinHandle<()>,         // Thread join handle
    path: PathBuf,                  // Path being watched
}
```

### 5.3 Key Functions and Their Contracts

#### Main Orchestration (`src/main.rs`)

**`main()`** → `Result<(), Box<dyn Error>>`
- **Location**: `src/main.rs:24`
- **Purpose**: Entry point; loads config, processes existing files, optionally starts daemon
- **Side effects**: Reads config file, processes files, spawns watcher threads

**`process_file()`** → `Result<(), Box<dyn Error>>`
- **Location**: `src/main.rs:137`
- **Purpose**: Process a single file - extract date, check GPS, move to target
- **Behavior**: Extracts date, determines target via GPS or default, builds path, moves file

**`determine_target()`** → `Result<(PathBuf, bool, bool, bool), Box<dyn Error>>`
- **Location**: `src/main.rs:176`
- **Purpose**: Determine target directory and hierarchy settings for a file
- **Returns**: `(target_path, nodecade, noyear, nomonth)`

#### Date Extraction (`src/date.rs`)

**`extract_date(path: &Path)`** → `Option<ExtractedDate>`
- **Location**: `src/date.rs:77`
- **Purpose**: Extract date from file using all methods (EXIF → Track → Filename)
- **Returns**: `ExtractedDate` with date string, source, and optional GPS

**`extract_date_from_filename(filename: &str)`** → `Option<ExtractedDate>`
- **Location**: `src/date.rs:196`
- **Purpose**: Extract date from filename using regex patterns
- **Patterns checked**: ISO prefix, ISO embedded, YYYY_MMDD, YYYYMMDD

**`is_valid_date(year, month, day)`** → `bool`
- **Location**: `src/date.rs:280`
- **Purpose**: Validate date components are within range (1990-2099)

**`decade_folder(year)`** → `String`
- **Location**: `src/date.rs:358`
- **Purpose**: Calculate decade folder name (e.g., "2020-2029")

#### File Operations (`src/files.rs`)

**`should_skip_file(path: &Path)`** → `Option<SkipReason>`
- **Location**: `src/files.rs:53`
- **Purpose**: Check if file should be skipped (directory, hidden, .sh/.rs)

**`scan_directory(dir: &Path)`** → `io::Result<Vec<PathBuf>>`
- **Location**: `src/files.rs:84`
- **Purpose**: Get all processable files from a directory

**`build_target_path(base, date, nodecade, noyear, nomonth)`** → `PathBuf`
- **Location**: `src/files.rs:106`
- **Purpose**: Build target path with date hierarchy

**`move_file_safe(source, target_dir, filename)`** → `io::Result<PathBuf>`
- **Location**: `src/files.rs:194`
- **Purpose**: Move file with collision handling (-N suffix)

**`resolve_collision(target_dir, filename)`** → `String`
- **Location**: `src/files.rs:158`
- **Purpose**: Generate unique filename if collision exists

#### Daemon Mode (`src/daemon.rs`)

**`run_daemon(dirs, processor_factory, config)`**
- **Location**: `src/daemon.rs:126`
- **Purpose**: Run daemon with file watching for all directories
- **Behavior**: Spawns watcher thread per directory, blocks until interrupted

**`watch_directory(dir, processor, config)`** → `WatchHandle`
- **Location**: `src/daemon.rs:60`
- **Purpose**: Watch a single directory for new files

#### GPS Processing (`src/gps.rs`)

**`calculate_distance(gps, target_coords)`** → `f64`
- **Location**: `src/gps.rs:95`
- **Purpose**: Calculate geodesic distance in meters

**`find_matching_place(gps, places)`** → `Option<PlaceMatch>`
- **Location**: `src/gps.rs:137`
- **Purpose**: Find first place matching GPS coordinates

#### Utilities

**`find_common_base(source, target)`** → `(PathBuf, PathBuf, PathBuf)`
- **Location**: `src/path.rs:15`
- **Purpose**: Find common base path for clean log output

**`load_config()`** → `Result<Config, Box<dyn Error>>`
- **Location**: `src/config.rs:118`
- **Purpose**: Load configuration from CLI and config file

### 5.4 Processing Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. FILE DETECTION                                               │
│    ├── Initial scan: fs::read_dir() on source directory        │
│    └── Daemon mode: notify watcher for Create/Rename events    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. FILE FILTERING                                               │
│    Skip if:                                                     │
│    ├── Is directory                                             │
│    ├── Starts with "." (hidden file)                           │
│    └── Extension is ".sh" or ".rs"                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. DATE EXTRACTION (in priority order)                         │
│    ├── 3a. EXIF metadata (images)                              │
│    │       Tags: DateTimeOriginal → CreateDate                  │
│    ├── 3b. Track metadata (videos)                             │
│    │       Tags: CreateDate                                     │
│    └── 3c. Filename patterns (see Section 6)                   │
│            Pattern 1: ^YYYY-MM-DD or ^YYYY_MM_DD               │
│            Pattern 2: embedded YYYY-MM-DD                       │
│            Pattern 3: YYYY_MMDD (e.g., 2020_0718)              │
│            Pattern 4: YYYYMMDD anywhere                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. GPS MATCHING (if GPS data present)                          │
│    For each configured place:                                   │
│    └── If geodesic_distance < place.radius → route to place    │
│    First matching place wins                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. TARGET PATH CONSTRUCTION                                     │
│    target_dir / [decade/] [year/] [month/] date / filename     │
│    Example: /Pictures/2020-2029/2024/03/2024-03-15/photo.jpg   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. FILE MOVE WITH COLLISION HANDLING                           │
│    ├── Create target directories if needed                     │
│    ├── If target exists: append -1, -2, etc.                   │
│    ├── 1-second delay before move (filesystem sync)            │
│    └── fs::rename(source, target)                              │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Date Extraction Logic

Date is determined using a priority-based approach. The first successful extraction wins.

**Module**: `src/date.rs`

### Priority 1: EXIF Metadata (Images)

**Handled by**: `extract_date_from_exif_with_parser()` at `src/date.rs:108`

| Tag Priority | EXIF Tag | Description |
|--------------|----------|-------------|
| 1 | `DateTimeOriginal` | Original capture date/time |
| 2 | `CreateDate` | File creation date |
| 3 | `ModifyDate` | Date of last modification (fallback) |

**EntryValue handling**: The parser handles multiple value types:
- `EntryValue::Time` - Direct datetime
- `EntryValue::NaiveDateTime` - Naive datetime without timezone
- Fallback to `as_time()` conversion for other types

**Output format**: `YYYY-MM-DD`

### Priority 2: Track Metadata (Videos)

**Handled by**: `extract_date_from_track_with_parser()` at `src/date.rs:156`

| Tag | Description |
|-----|-------------|
| `CreateDate` | Video creation date |

### Priority 3: Filename Patterns

**Handled by**: `extract_date_from_filename()` at `src/date.rs:196`

Checked in the following order (first match wins):

| Priority | Pattern Name | Regex | Example Filename | Extracted Date |
|----------|--------------|-------|------------------|----------------|
| 1 | ISO prefix | `^(\d{4}[-_]\d{2}[-_]\d{2})` | `2024-03-15_photo.jpg` | 2024-03-15 |
| 2 | ISO embedded | `[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})` | `IMG_2024-03-15_photo.jpg` | 2024-03-15 |
| 3 | YYYY_MMDD | `(\d{4})_(\d{4})` | `2020_0718_064509.MP4` | 2020-07-18 |
| 4 | YYYYMMDD | `(\d{8})` | `IMG_20240315_120000.jpg` | 2024-03-15 |

**Note**: The ISO embedded pattern requires a non-digit, non-hyphen character before the date (e.g., underscore or letter).

**Regex definitions** (`src/date.rs:44-60`, compiled once via `OnceLock`):
```rust
static YYYY_MM_DD_PREFIX: OnceLock<Regex> = OnceLock::new();
static YYYY_MM_DD_EMBEDDED: OnceLock<Regex> = OnceLock::new();
static YYYYMMDD: OnceLock<Regex> = OnceLock::new();
static YYYY_MMDD: OnceLock<Regex> = OnceLock::new();
```

### Validation Rules

**Location**: `is_valid_date()` at `src/date.rs:280`, `try_parse_yyyymmdd()` at `src/date.rs:305`, `try_parse_yyyy_mmdd()` at `src/date.rs:334`

| Field | Valid Range | Notes |
|-------|-------------|-------|
| Year | 1990 - 2099 | Prevents false positives from random 8-digit numbers |
| Month | 1 - 12 | |
| Day | 1 - 31 | Further validated by chrono for actual date validity |

**Leap year handling**: `chrono::NaiveDate::from_ymd_opt()` validates February 29 correctly.

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| EXIF date is `0000:00:00` | Falls back to filename patterns |
| No date found | File is skipped (not moved) |
| Multiple patterns match | First pattern in priority order wins |
| Invalid date (e.g., Feb 30) | Pattern rejected, tries next pattern |
| Underscore in date separators | Treated same as hyphen (`_` → `-`) |

---

## 7. GPS and Geolocation

### Coordinate System

- **Standard**: WGS84 (World Geodetic System 1984)
- **Format**: Decimal degrees (not DMS)
- **Storage in config**: `lat` (latitude), `lon` (longitude)

### Distance Calculation

**Method**: Geodesic (great-circle) distance using Vincenty's formula via `geo` crate

**Location**: `calculate_distance()` at `src/gps.rs:91`

```rust
pub fn calculate_distance(gps: &GPSInfo, target_coords: &(f64, f64)) -> f64 {
    let gps_point = gps_to_point(gps);
    let target = Point::new(target_coords.1, target_coords.0); // (lon, lat)
    Geodesic.distance(gps_point, target)  // Returns meters
}
```

**Important**: Function returns **meters**; caller divides by 1000 for km comparison.

### GPS Coordinate Conversion

**Location**: `gps_to_point()` at `src/gps.rs:55`

EXIF GPS data is stored as degrees/minutes/seconds (DMS). Conversion handles:
- DMS to decimal degrees conversion
- Hemisphere reference signs (`N`/`S` for latitude, `E`/`W` for longitude)

### Place Matching Logic

**Location**: `find_matching_place()` at `src/gps.rs:131`

```rust
pub fn find_matching_place<'a>(gps: &GPSInfo, places: &'a [Place]) -> Option<PlaceMatch<'a>> {
    for place in places {
        let pos: (f64, f64) = (place.lat, place.lon);
        let distance = calculate_distance(gps, &pos) / 1000.0; // Convert to km
        if distance < place.radius {
            return Some(PlaceMatch { place, distance_km: distance });
        }
    }
    None
}
```

### Overlapping Place Radii

**Behavior**: First matching place in config order wins.

**Recommendation**: Order places from most specific (smallest radius) to least specific in your config file.

### Performance Considerations

- GPS matching is O(n) where n = number of configured places
- Distance calculation involves trigonometric functions (relatively expensive)
- For large numbers of places (>100), consider spatial indexing (not currently implemented)

---

## 8. Error Handling Strategy

### Error Propagation

The application uses `Result<(), Box<dyn Error>>` for error handling. Most errors propagate up to `main()` and cause program termination.

### Error Types by Source

| Operation | Error Type | Handling |
|-----------|------------|----------|
| Config file not found | Prints warning | Uses empty defaults, continues |
| Config parse error | `toml::de::Error` | Propagates, terminates |
| File I/O error | `std::io::Error` | Propagates (batch) or prints (single file) |
| Regex compilation | `regex::Error` | Propagates, terminates |
| EXIF parsing | nom-exif error | Prints message, falls back to filename |
| Date validation | Returns `None` | Tries next pattern |

### Logging

**Current implementation**: Uses `println!` for all output.

| Event | Output |
|-------|--------|
| Startup | Lists watched directories and places |
| File move | `Move: "base"/{\"rel_source\" -> \"rel_target\"}... success/error` |
| GPS match | `Found picture from "place_name", distance: X.XXkm` |
| File skip | `Skip: "/path/to/file"` (daemon mode) |
| EXIF error | `Cannot parse: <error>` |
| Watch error | `watch error: <error>` |

### Files That Fail Processing

Files that fail to process (no date extracted, I/O error) are:
- **Not moved**: Left in original location
- **Not deleted**: Non-destructive operation
- **Logged**: Error printed to stdout

---

## 9. Testing Guide

### 9.1 Test Organization

Tests are located in module-specific `#[cfg(test)]` blocks. Total: **50 tests**

```
src/date.rs (18 tests)
└── mod tests
    ├── test_is_valid_date_valid, test_is_valid_date_invalid_year/month/day
    ├── test_is_valid_date_leap_year
    ├── test_is_valid_date_str_valid, test_is_valid_date_str_invalid_format
    ├── test_try_parse_yyyymmdd_valid/invalid
    ├── test_try_parse_yyyy_mmdd_valid/invalid
    ├── test_extract_date_from_filename_* (prefix, underscore, embedded, yyyymmdd, yyyy_mmdd, no_match)
    ├── test_decade_folder, test_month_folder
    └── test_format_date

src/files.rs (13 tests)
└── mod tests
    ├── test_should_skip_hidden_file, test_should_skip_script_files, test_should_not_skip_normal_files
    ├── test_build_target_path_* (full_hierarchy, no_decade, no_year, no_month, no_hierarchy)
    ├── test_resolve_collision_* (no_conflict, single, multiple, no_extension)
    ├── test_scan_directory_filters_correctly
    └── test_move_file_quiet_* (creates_dirs, handles_collision)

src/daemon.rs (5 tests)
└── mod tests
    ├── test_should_process_event_create_file
    ├── test_should_process_event_rename_to
    ├── test_should_process_event_other_events
    └── test_daemon_config_default

src/gps.rs (3 tests)
└── mod tests
    ├── test_gps_coordinates_new
    ├── test_distance_km_same_point
    └── test_distance_km_different_points

src/path.rs (8 tests + 2 Windows-only)
└── mod tests
    ├── test_common_parent_directory
    ├── test_one_is_prefix_of_other
    ├── test_target_is_prefix_of_source
    ├── test_identical_paths
    ├── test_divergent_paths
    ├── test_completely_different_paths
    ├── test_root_paths
    ├── test_deep_nesting
    ├── test_different_drives_windows  (#[cfg(windows)])
    └── test_same_drive_windows        (#[cfg(windows)])
```

### 9.2 Running Specific Tests

```bash
# Run all tests (50 tests)
cargo test

# Run tests in a specific module
cargo test date::tests
cargo test files::tests
cargo test daemon::tests
cargo test gps::tests
cargo test path::tests

# Run a specific test
cargo test test_extract_date_from_filename_prefix

# Run with output visible
cargo test test_build_target_path_full_hierarchy -- --nocapture

# Run all tests matching a pattern
cargo test collision
```

### 9.3 Test Coverage

**Comprehensive coverage**:
- Date extraction from filenames (all patterns)
- Date validation (valid/invalid cases, leap years)
- File filtering logic (hidden, extensions)
- Target path building (all hierarchy combinations)
- File collision handling
- Directory scanning
- Event filtering in daemon mode
- GPS coordinate handling
- Path utilities

**Not currently tested** (would require test fixtures):
- EXIF parsing from actual image files
- Video track metadata parsing
- Config file loading (integration test)

### 9.4 Writing New Tests

Tests use temporary directories with atomic counters for isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "sort_pictures_test_{}_{}",
            std::process::id(),
            id
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_new_feature() {
        let temp = temp_dir();
        // Test code here...
    }
}
```

---

## 10. Daemon Mode and Systemd Integration

### How File Watching Works

**Module**: `src/daemon.rs`

1. For each configured directory, `watch_directory()` spawns a dedicated thread
2. Each thread creates a `notify::recommended_watcher`
3. Watcher monitors for `Create(File)` and `Modify(Name(RenameMode::To))` events
4. Events are filtered by `should_process_event()` before calling the processor

```rust
// Events that trigger processing (src/daemon.rs:94-103):
fn should_process_event(event: &Event) -> Option<&Path> {
    match event.kind {
        EventKind::Create(CreateKind::File) => event.paths.first().map(|p| p.as_path()),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => event.paths.first().map(|p| p.as_path()),
        _ => None,
    }
}
```

### Thread Model

```
main thread
    │
    ├── Initial scan (sequential)
    │   └── process_directory() for each dir
    │
    └── Daemon mode (daemon::run_daemon)
        ├── Thread 1: watch_directory(dir[0], processor)
        ├── Thread 2: watch_directory(dir[1], processor)
        └── ...
        └── Thread N: watch_directory(dir[N-1], processor)
```

**Processor pattern**: `main.rs` creates a closure for each directory that captures the configuration:
```rust
daemon::run_daemon(&dirs, |dir| {
    let places = places.clone();
    let target = dir.target.clone();
    // ... capture other config
    move |path: &Path| {
        process_file(path, &target, nodecade, noyear, nomonth, &places)
    }
}, daemon::DaemonConfig::default());
```

### Graceful Shutdown

**Current behavior**: No explicit graceful shutdown. Process terminates on:
- SIGTERM/SIGINT (systemd stop)
- Thread panic (unwrap failures)

`WatchHandle::join()` is called for each watcher but they run indefinitely.

### Systemd Unit File

**Location**: `systemd/sort_pictures.service`

```ini
[Unit]
Description=Sort Pictures Service
After=graphical-session.target

[Service]
Type=simple
ExecStart=/home/locutus/apps/bin/sort_pictures --daemonize
Restart=always
RestartSec=10
Environment=HOME=%h
WorkingDirectory=%h

[Install]
WantedBy=default.target
```

**Key settings**:
- `Type=simple`: Process runs in foreground
- `Restart=always`: Auto-restart on crash
- `RestartSec=10`: Wait 10s between restart attempts
- User service: Runs as current user, not root

### Installation Procedure

**Script**: `install.sh`

```bash
./install.sh
```

**Steps performed**:
1. Build release binary (`cargo build --release`)
2. Create directories: `~/apps/bin/`, `~/.config/systemd/user/`, `~/.config/sort_pictures/`
3. Stop existing service if running
4. Copy binary to `~/apps/bin/sort_pictures` (uses `$(pwd)` for reliable paths)
5. Copy service file to `~/.config/systemd/user/`
6. Copy example config to `~/.config/sort_pictures/config.toml`
7. Reload systemd, enable and start service

### Service Management Commands

```bash
# View status
systemctl --user status sort_pictures

# Stop service
systemctl --user stop sort_pictures

# Start service
systemctl --user start sort_pictures

# Restart service
systemctl --user restart sort_pictures

# Disable auto-start
systemctl --user disable sort_pictures

# View logs (follow mode)
journalctl --user -u sort_pictures -f

# View recent logs
journalctl --user -u sort_pictures -n 100
```

### Log Location

Logs go to systemd journal. Access via `journalctl --user -u sort_pictures`.

---

## 11. Development Workflows

### 11.1 Adding a New Date Pattern

**Files to modify**: `src/date.rs`

1. **Add regex static** (in the static definitions section):
```rust
static NEW_PATTERN: OnceLock<Regex> = OnceLock::new();

fn new_pattern_regex() -> &'static Regex {
    NEW_PATTERN.get_or_init(|| Regex::new(r"your_pattern_here").unwrap())
}
```

2. **Add extraction logic** in `extract_date_from_filename()`:
```rust
// Pattern N: Description
if let Some(captures) = new_pattern_regex().captures(filename) {
    // Extract year, month, day from captures
    // Call validation function
    // Return Some(ExtractedDate { ... })
}
```

3. **Add validation function** if needed (similar to `try_parse_yyyymmdd`).

4. **Add test cases** in `src/date.rs` tests:
```rust
#[test]
fn test_extract_date_from_filename_new_pattern() {
    let result = extract_date_from_filename("example_filename.jpg");
    assert!(result.is_some());
    assert_eq!(result.unwrap().date, "2024-03-15");
}
```

### 11.2 Adding a New Configuration Option

**Files to modify**: `src/config.rs`, then consuming modules

1. **Add field to struct** (`Dir`, `Place`, or `Config`):
```rust
pub struct Dir {
    // existing fields...
    #[serde(default)]
    pub new_option: bool,
}
```

2. **Update processing logic** - depending on the option:
   - Date-related: Update `src/date.rs`
   - File operations: Update `src/files.rs`
   - Daemon behavior: Update `src/daemon.rs`
   - Orchestration: Update `src/main.rs`

3. **Update example config** in `systemd/config.toml`.

4. **Update this CLAUDE.md** in the Configuration section.

### 11.3 Adding File Filtering Logic

**Files to modify**: `src/files.rs`

1. **Update `should_skip_file()`** to add new skip conditions:
```rust
pub fn should_skip_file(path: &Path) -> Option<SkipReason> {
    // existing checks...

    // New check
    if some_condition(path) {
        return Some(SkipReason::NewReason);
    }

    None
}
```

2. **Add `SkipReason` variant** if needed:
```rust
pub enum SkipReason {
    // existing variants...
    NewReason,
}
```

3. **Add tests** for the new filter.

### 11.4 Debugging File Processing Issues

**To trace why a specific file was handled a certain way:**

1. **Run in non-daemon mode** for single-pass processing with output:
```bash
cargo run -- --config your_config.toml
```

2. **Add debug prints** in `src/main.rs:process_file()`:
```rust
println!("Processing: {:?}", file_path);
let extracted = date::extract_date(file_path);
println!("Extracted date: {:?}", extracted);
```

3. **Check file manually** for EXIF data:
```bash
exiftool your_file.jpg
```

4. **Test date extraction directly**:
```rust
#[test]
fn debug_specific_file() {
    let result = extract_date_from_filename("your_filename.jpg");
    println!("Result: {:?}", result);
}
```
Run with: `cargo test debug_specific_file -- --nocapture`

---

## 12. Dependencies Rationale

| Crate | Version | Purpose | Alternatives |
|-------|---------|---------|--------------|
| `chrono` | 0.4.41 | Date validation and manipulation | `time` crate (similar, slightly different API) |
| `clap` | 4.5.39 | CLI argument parsing with derive macros | `structopt` (older, merged into clap), `argh` (simpler) |
| `dirs` | 6.0.0 | Platform-specific config directory resolution | `directories` (more comprehensive), manual `$HOME` |
| `geo` | 0.30.0 | Geodesic distance calculations | `geoutils`, manual Haversine formula |
| `nom-exif` | 2.5.4 | EXIF and video track metadata parsing | `kamadak-exif`, `rexif` (less maintained) |
| `notify` | 8.0.0 | Cross-platform file system watching | `hotwatch` (wrapper), `inotify` (Linux-only) |
| `regex` | 1.11.1 | Filename pattern matching | `onig` (different engine), manual parsing |
| `serde` | 1.0.219 | Serialization/deserialization framework | `nanoserde` (simpler, less features) |
| `toml` | 0.8.23 | TOML config file parsing | `toml_edit` (preserves formatting), `config` (multi-format) |

### Version Constraints

No strict version constraints beyond what's in `Cargo.toml`. All dependencies use caret versioning (default), allowing minor and patch updates.

---

## 13. Known Limitations and Gotchas

### Limitations

| Limitation | Impact | Workaround |
|------------|--------|------------|
| Single-level directory watching | Subdirectories not monitored | Create separate `[[dirs]]` entries |
| First GPS match wins | May misroute if radii overlap | Order places by specificity in config |
| No remote filesystem support | Network drives may have issues | Use local mounts |
| Files without dates skipped | Some files left unsorted | Ensure files have EXIF or named correctly |
| `.sh` and `.rs` files always skipped | Cannot sort script files | Rename or use different extension |

### Edge Cases

| Case | Behavior |
|------|----------|
| File created then immediately deleted | May error on processing |
| Very long filenames | Collision suffix adds more length |
| Non-UTF8 filenames | `to_string_lossy()` replaces invalid chars |
| Symlinks | Followed (not treated specially) |
| Read-only target directory | Error on move |
| File still being written | 1-second delay helps avoid partial reads |

### Common Mistakes

1. **Forgetting `--daemonize`**: Program exits after initial scan
2. **Source directory doesn't exist**: Error on startup
3. **Target path not absolute**: May create relative directories
4. **Overlapping GPS places**: First match wins (may be unexpected)
5. **Missing `target` for places**: Required field (unlike dirs)

---

## 14. Code Style and Conventions

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Functions | `snake_case` | `process_fname`, `is_valid_date` |
| Structs | `PascalCase` | `Config`, `Dir`, `Place` |
| Variables | `snake_case` | `file_date_map`, `date_found` |
| Constants | `SCREAMING_SNAKE_CASE` | `GLOBAL_PARAMS` |

### Comment Language

**English**. Comments use English throughout the codebase:

```rust
// Create regex patterns for various date formats

// Skip directories and hidden files
```

### Error Messages

- Use English for error messages
- Keep messages concise but descriptive
- Include context (file path, expected format)

### Logging

- Use `println!` for all output (no log crate)
- Include file paths in quotes
- Show success/error status for operations

---

## 15. Contributing Guidelines (for Claude Code)

### Before Modifying Code

1. **Read the relevant functions** before making changes
2. **Understand the data flow** through the processing pipeline
3. **Check existing patterns** for similar functionality
4. **Verify config structure** if adding new options

### Maintaining Backward Compatibility

- **Config files**: New fields should have `#[serde(default)]`
- **CLI args**: Use `#[arg(skip)]` for config-only options
- **File operations**: Never delete files, always move

### Required Test Coverage

- **New date patterns**: Add test cases for valid and invalid inputs
- **New config options**: Verify default values work correctly
- **Path operations**: Test edge cases (empty paths, root paths)

### Documentation Expectations

- **New functions**: Add doc comments explaining purpose, inputs, outputs
- **New config options**: Update this CLAUDE.md and example config
- **Complex logic**: Add inline comments explaining the "why"

### Code Review Checklist

- [ ] No unwrap() on user input (use proper error handling)
- [ ] New regex patterns are validated
- [ ] Date validation uses chrono for correctness
- [ ] File operations check existence before acting
- [ ] Config changes include serde attributes

---

## Appendix: Quick Reference

### File Extensions Skipped
- Hidden files (starting with `.`)
- `.sh` files
- `.rs` files

### Date Validation Range
- **Year**: 1990 - 2099
- **Month**: 1 - 12
- **Day**: 1 - 31 (chrono-validated)

### Directory Hierarchy
```
[target]/[decade]/[year]/[month]/[date]/[filename]
```

### Config File Priority
1. CLI `--config` argument
2. `~/.config/sort_pictures/config.toml`

### Key File Locations
- **Binary**: `~/apps/bin/sort_pictures` (after install)
- **Config**: `~/.config/sort_pictures/config.toml`
- **Service**: `~/.config/systemd/user/sort_pictures.service`
- **Logs**: `journalctl --user -u sort_pictures`
