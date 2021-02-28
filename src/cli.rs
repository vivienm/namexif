use std::path::PathBuf;

use chrono_tz::Tz;
use structopt::StructOpt;

/// Rename photos according to their EXIF date tag
#[derive(Debug, StructOpt)]
#[structopt(global_setting = structopt::clap::AppSettings::ColoredHelp)]
pub struct Args {
    /// Does not prompt for confirmation
    #[structopt(short = "y", long = "assume-yes")]
    pub assume_yes: bool,
    /// Does not actually rename files
    #[structopt(short = "n", long = "dry-run")]
    pub dry_run: bool,
    /// Log verbosity level
    #[structopt(
        short = "l",
        long = "log-level",
        value_name = "level",
        env = "NAMEXIF_LOG_LEVEL",
        default_value = "info",
        possible_values = &["off", "error", "warn", "info", "debug", "trace"]
    )]
    pub log_level: log::LevelFilter,
    /// Filename format
    #[structopt(
        short = "f",
        long = "format",
        value_name = "format",
        env = "NAMEXIF_FORMAT",
        default_value = "%Y-%m-%dT%H:%M:%S%z"
    )]
    pub name_format: String,
    /// Time zone
    #[structopt(short = "z", long = "timezone", env = "NAMEXIF_TIMEZONE")]
    pub timezone: Option<Tz>,
    /// Input file or directory
    #[structopt(value_name = "input", default_value = ".", parse(from_os_str))]
    pub source_path: PathBuf,
}
