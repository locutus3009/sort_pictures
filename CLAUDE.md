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
│   ├── main.rs                   # Entry point, file processing logic, daemon mode
│   ├── config.rs                 # Configuration structs, CLI args, config loading
│   ├── gps.rs                    # GPS coordinate extraction and distance calculations
│   └── path.rs                   # Path utilities (find_common_base)
└── systemd/
    ├── config.toml               # Example configuration file
    └── sort_pictures.service     # Systemd unit file template
```

### Source File Responsibilities

| File | Lines | Purpose |
|------|-------|---------|
| `src/main.rs` | ~581 | Entry point, file processing orchestration, date extraction, daemon mode |
| `src/config.rs` | ~152 | Configuration structs (`Config`, `Dir`, `Place`), CLI parsing, config file loading |
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
main.rs
    │
    ├── config.rs (Config, Dir, Place, load_config)
    │   ├── clap (CLI argument parsing)
    │   ├── serde/toml (config file deserialization)
    │   └── dirs (config path resolution)
    │
    ├── gps.rs (GPS processing)
    │   ├── geo (geodesic distance calculations)
    │   ├── nom-exif (GPSInfo, LatLng, URational types)
    │   └── config.rs (Place struct for matching)
    │
    ├── path.rs
    │   └── find_common_base() (logging utility)
    │
    ├── Date Extraction (in main.rs)
    │   ├── nom-exif (EXIF/Track parsing)
    │   ├── regex (filename patterns)
    │   └── chrono (date validation)
    │
    ├── File Operations (in main.rs)
    │   └── std::fs (read_dir, rename, create_dir_all)
    │
    └── Daemon Mode (in main.rs)
        ├── notify (file watching)
        └── std::thread/mpsc (threading)
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

### 5.3 Key Functions and Their Contracts

#### `main()` → `Result<(), Box<dyn Error>>`
**Location**: `src/main.rs:297`

- **Purpose**: Entry point; loads config, processes existing files, optionally starts daemon
- **Side effects**:
  - Reads config file (via `config::load_config()`)
  - Processes and moves files in configured directories
  - Spawns watcher threads if `--daemonize`
- **Thread safety**: Uses `Mutex<Config>` for global config access

#### `process_fname()` → `Result<(), Box<dyn Error>>`
**Location**: `src/main.rs:41`

```rust
fn process_fname(
    nodecade: bool,
    noyear: bool,
    nomonth: bool,
    fname: PathBuf,           // File or directory to process
    target_dir: &Option<PathBuf>,
    places: &[Place],
) -> Result<(), Box<dyn Error>>
```

- **Purpose**: Core processing logic for a file or directory
- **Input**: Can accept a single file path OR a directory path
- **Behavior**:
  - If directory: iterates over all files in it
  - If file: processes only that file
  - Extracts date via EXIF → Track → filename patterns
  - Checks GPS against configured places
  - Moves file to appropriate target with hierarchy
- **Side effects**: Creates directories, moves files
- **Error handling**: Returns error on I/O failures, regex failures

#### `parse_exif()` → `Option<(String, Option<GPSInfo>)>`
**Location**: `src/main.rs:439`

- **Purpose**: Extract date and GPS from EXIF metadata
- **Returns**: `Some((date_string, gps_info))` or `None` if parsing fails
- **Date format returned**: `"YYYY-MM-DD"`
- **Tags checked**: `DateTimeOriginal`, `CreateDate`, `ModifyDate` (in priority order)
- **EntryValue handling**: Supports `Time`, `NaiveDateTime`, and `as_time()` fallback
- **GPS handling**: Uses `.ok().flatten()` for robust GPS extraction (won't panic on missing GPS)

#### `parse_track()` → `Option<(String, Option<GPSInfo>)>`
**Location**: `src/main.rs:482`

- **Purpose**: Extract date and GPS from video track metadata
- **Returns**: `Some((date_string, gps_info))` or `None`
- **Tags checked**: `CreateDate`

#### `is_valid_date(date_str: &str)` → `bool`
**Location**: `src/main.rs:512`

- **Purpose**: Validate date string format and values
- **Input format**: `"YYYY-MM-DD"` (exactly 10 characters)
- **Validation**: Year 1990-2099, month 1-12, day 1-31, chrono validation for actual date validity

#### `calculate_distance(gps: &GPSInfo, target_coords: &(f64, f64))` → `f64`
**Location**: `src/gps.rs:91`

- **Purpose**: Calculate geodesic distance between GPS point and target coordinates
- **Returns**: Distance in **meters**
- **Note**: Caller divides by 1000 to compare against radius (in km)

#### `find_matching_place(gps: &GPSInfo, places: &[Place])` → `Option<PlaceMatch>`
**Location**: `src/gps.rs:131`

- **Purpose**: Find the first configured place matching the GPS coordinates
- **Returns**: `Some(PlaceMatch)` with the matched place and distance, or `None`
- **Note**: First matching place in config order wins

#### `find_common_base(source: &Path, target: &Path)` → `(PathBuf, PathBuf, PathBuf)`
**Location**: `src/path.rs:15`

- **Purpose**: Find common base path for clean log output
- **Returns**: `(base_path, relative_source, relative_target)`
- **Example**: `/home/user/a/file.txt` + `/home/user/b/photo.jpg` → `(/home/user, a/file.txt, b/photo.jpg)`

#### `load_config()` → `Result<Config, Box<dyn Error>>`
**Location**: `src/config.rs:104`

- **Purpose**: Load configuration from CLI arguments and config file
- **Returns**: Merged configuration with CLI args taking precedence
- **Default config path**: `~/.config/sort_pictures/config.toml`

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

### Priority 1: EXIF Metadata (Images)

**Handled by**: `parse_exif()` at `src/main.rs:439`

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

**Handled by**: `parse_track()` at `src/main.rs:482`

| Tag | Description |
|-----|-------------|
| `CreateDate` | Video creation date |

### Priority 3: Filename Patterns

Checked in the following order (first match wins):

| Priority | Pattern Name | Regex | Example Filename | Extracted Date |
|----------|--------------|-------|------------------|----------------|
| 1 | ISO prefix | `^(\d{4}[-_]\d{2}[-_]\d{2})` | `2024-03-15_photo.jpg` | 2024-03-15 |
| 2 | ISO embedded | `[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})` | `IMG-2024-03-15-WA001.jpg` | 2024-03-15 |
| 3 | YYYY_MMDD | `(\d{4})_(\d{4})` | `2020_0718_064509.MP4` | 2020-07-18 |
| 4 | YYYYMMDD | `(\d{8})` | `IMG_20240315_120000.jpg` | 2024-03-15 |

**Regex definitions** (`src/main.rs:49-53`):
```rust
let yyyy_mm_dd_prefix_regex = Regex::new(r"^(\d{4}[-_]\d{2}[-_]\d{2})")?;
let yyyy_mm_dd_embedded_regex = Regex::new(r"[^0-9-](\d{4}[-_]\d{2}[-_]\d{2})")?;
let yyyymmdd_regex = Regex::new(r"(\d{8})")?;
let yyyy_mmdd_regex = Regex::new(r"(\d{4})_(\d{4})")?;
```

### Validation Rules

**Location**: `is_valid_date()` at `src/main.rs:512`, `try_parse_yyyymmdd()` at `src/main.rs:534`, `try_parse_yyyy_mmdd()` at `src/main.rs:560`

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

Tests are located in module-specific `#[cfg(test)]` blocks:

```
src/path.rs
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

src/gps.rs
└── mod tests
    ├── test_gps_coordinates_new
    ├── test_distance_km_same_point
    └── test_distance_km_different_points
```

### 9.2 Running Specific Tests

```bash
# Run all tests
cargo test

# Run all path module tests
cargo test path::tests

# Run all GPS module tests
cargo test gps::tests

# Run a specific test
cargo test test_common_parent_directory

# Run with output visible
cargo test test_common_parent_directory -- --nocapture

# Run all tests matching a pattern
cargo test common
```

### 9.3 Test Coverage

Currently tested:
- Path utility functions (`find_common_base`)
- Various path relationship scenarios
- Cross-platform path handling (Windows conditional tests)
- GPS coordinate creation and distance calculations

**Not currently tested** (potential areas for expansion):
- Date extraction from filenames
- EXIF parsing (would require test fixtures)
- Config file parsing
- File collision handling

### 9.4 Writing New Tests

**For filename date extraction:**
```rust
#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn test_yyyymmdd_extraction() {
        assert_eq!(
            try_parse_yyyymmdd("20240315"),
            Some("2024-03-15".to_string())
        );
    }

    #[test]
    fn test_invalid_date() {
        assert_eq!(try_parse_yyyymmdd("20241315"), None); // Invalid month
    }
}
```

---

## 10. Daemon Mode and Systemd Integration

### How File Watching Works

**Location**: `src/main.rs:354-435`

1. For each configured directory, spawns a dedicated thread
2. Each thread creates a `notify::recommended_watcher`
3. Watcher monitors for `Create(File)` and `Modify(Name(RenameMode::To))` events
4. Events trigger `process_fname()` for the affected file

```rust
// Events that trigger processing:
EventKind::Create(CreateKind::File)
EventKind::Modify(ModifyKind::Name(RenameMode::To))  // File renamed into directory
```

### Thread Model

```
main thread
    │
    ├── Initial scan (sequential)
    │   └── process_fname() for each dir
    │
    └── Daemon mode
        ├── Thread 1: watcher for dir[0]
        ├── Thread 2: watcher for dir[1]
        └── ...
        └── Thread N: watcher for dir[N-1]
```

**Synchronization**: Global config in `Mutex<Config>` (read-only after init)

### Graceful Shutdown

**Current behavior**: No explicit graceful shutdown. Process terminates on:
- SIGTERM/SIGINT (systemd stop)
- Thread panic (unwrap failures)

Threads are joined in `main()` but watchers run indefinitely.

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

**Files to modify**: `src/main.rs`

1. **Add regex constant** (around line 49-53):
```rust
let new_pattern_regex = Regex::new(r"your_pattern_here")?;
```

2. **Add extraction logic** (in `process_fname()`, after the existing date pattern checks):
```rust
if date_found.is_none()
    && let Some(captures) = new_pattern_regex.captures(&filename) {
    // Extract year, month, day from captures
    // Call validation function
    // Set date_found = Some(date_str)
}
```

3. **Add validation function** if needed (similar to `try_parse_yyyymmdd`).

4. **Add test cases** for the new pattern.

### 11.2 Adding a New Configuration Option

**Files to modify**: `src/config.rs`

1. **Add field to struct** (`Dir`, `Place`, or `Config`):
```rust
pub struct Dir {
    // existing fields...
    #[serde(default)]
    pub new_option: bool,
}
```

2. **Update processing logic** in `src/main.rs` to use the new option.

3. **Update example config** in `systemd/config.toml`.

4. **Update this CLAUDE.md** in the Configuration section.

### 11.3 Adding GPS Matching Logic

**Files to modify**: `src/gps.rs`

1. **Add new GPS-related function** in `src/gps.rs`:
```rust
/// Your new GPS utility function
pub fn new_gps_function(/* params */) -> ReturnType {
    // Implementation
}
```

2. **Export from module** - ensure the function is `pub` if needed by other modules.

3. **Import in main.rs** if used there:
```rust
use gps::new_gps_function;
```

4. **Add test cases** in `src/gps.rs` under `#[cfg(test)] mod tests`.

### 11.4 Debugging File Processing Issues

**To trace why a specific file was handled a certain way:**

1. **Run in non-daemon mode** for single-pass processing with output:
```bash
cargo run -- --config your_config.toml
```

2. **Add debug prints** in `process_fname()`:
```rust
println!("Processing: {:?}", filename);
println!("EXIF date: {:?}", date_found);
println!("GPS data: {:?}", gps_data);
```

3. **Check file manually** for EXIF data:
```bash
exiftool your_file.jpg
```

4. **Verify filename pattern match**:
```bash
# Test regex in Rust playground or add a test
```

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
