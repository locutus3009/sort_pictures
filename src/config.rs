//! Configuration module for sort_pictures.
//!
//! This module handles configuration structures, CLI argument parsing,
//! and loading configuration from TOML files.

use clap::{CommandFactory, Parser};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

/// Directory monitoring configuration.
///
/// Defines a source directory to watch and optional target directory
/// for organized files, along with hierarchy options.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Dir {
    /// Source directory to monitor files.
    #[serde(default)]
    pub source: Option<PathBuf>,
    /// Target directory to save files.
    #[serde(default)]
    pub target: Option<PathBuf>,
    /// Disable creation of "decade" directory (e.g., 2020-2029/).
    #[serde(default)]
    pub nodecade: bool,
    /// Disable creation of "year" directory.
    #[serde(default)]
    pub noyear: bool,
    /// Disable creation of "month" directory.
    #[serde(default)]
    pub nomonth: bool,
}

/// GPS-based place routing configuration.
///
/// Defines a geographic location that, when matched against photo GPS data,
/// routes photos to a specific target directory.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Place {
    /// Target directory to save files.
    #[serde(default)]
    pub target: Option<PathBuf>,
    /// Disable creation of "decade" directory.
    #[serde(default)]
    pub nodecade: bool,
    /// Disable creation of "year" directory.
    #[serde(default)]
    pub noyear: bool,
    /// Disable creation of "month" directory.
    #[serde(default)]
    pub nomonth: bool,
    /// Radius to detect images (in kilometers).
    #[serde(default)]
    pub radius: f64,
    /// Place's longitude in decimal degrees (WGS84).
    #[serde(default)]
    pub lon: f64,
    /// Place's latitude in decimal degrees (WGS84).
    #[serde(default)]
    pub lat: f64,
    /// Place's human-readable name.
    #[serde(default)]
    pub name: String,
}

/// Main configuration structure combining CLI arguments and file config.
#[derive(Parser, Deserialize, Serialize, Clone, Debug)]
#[command(name = "sort_pictures")]
#[command(about = "A program to re-order pictures in the directory")]
pub struct Config {
    /// Config file path (default: ~/.config/sort_pictures/config.toml)
    #[arg(short, long)]
    #[serde(skip)]
    pub config: Option<PathBuf>,

    /// Run as a daemon.
    #[arg(long)]
    #[serde(skip)]
    pub daemonize: bool,

    /// Directory configurations
    #[arg(skip)]
    #[serde(default)]
    pub dirs: Vec<Dir>,

    /// Places configurations
    #[arg(skip)]
    #[serde(default)]
    pub places: Vec<Place>,
}

impl Config {
    /// Creates an empty configuration with default values.
    pub const fn empty() -> Self {
        Self {
            config: None,
            daemonize: false,
            dirs: Vec::new(),
            places: Vec::new(),
        }
    }
}

/// Returns the default configuration file path.
///
/// The default path is `~/.config/sort_pictures/config.toml`.
pub fn default_config_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut path = dirs::config_dir().ok_or("Cannot get config dir")?;
    path.push("sort_pictures");
    std::fs::create_dir_all(&path)?;
    path.push("config.toml");
    Ok(path)
}

/// Loads configuration from CLI arguments and config file.
///
/// CLI arguments take precedence over config file values when explicitly provided.
pub fn load_config() -> Result<Config, Box<dyn Error>> {
    // Parse CLI first to get config file path
    let cli_args = Config::parse();

    let config_path = match &cli_args.config {
        Some(path) => path.clone(),
        None => default_config_path()?,
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
