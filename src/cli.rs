use std::path::PathBuf;

use chrono_tz::Tz;
use clap_complete::Shell;

/// Rename photos according to their EXIF date tag
#[derive(Debug, clap::Parser)]
pub struct Args {
    /// Does not prompt for confirmation
    #[clap(short = 'y', long = "assume-yes")]
    pub assume_yes: bool,
    /// Does not actually rename files
    #[clap(short = 'n', long = "dry-run")]
    pub dry_run: bool,
    /// Generates a completion file
    #[clap(long = "completion")]
    pub completion: Option<Shell>,
    /// Log verbosity level
    #[clap(
        short = 'l',
        long = "log-level",
        value_name = "level",
        env = "NAMEXIF_LOG_LEVEL",
        default_value = "info"
    )]
    pub log_level: log::LevelFilter,
    /// Filename format
    #[clap(
        short = 'f',
        long = "format",
        value_name = "format",
        env = "NAMEXIF_FORMAT",
        default_value = "%Y-%m-%dT%H:%M:%S%z"
    )]
    pub name_format: String,
    /// Time zone
    #[clap(short = 'z', long = "timezone", env = "NAMEXIF_TIMEZONE")]
    pub timezone: Option<Tz>,
    /// Input file or directory
    #[clap(value_name = "input", default_value = ".")]
    pub source_path: PathBuf,
}
