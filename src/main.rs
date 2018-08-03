extern crate chrono;
extern crate chrono_tz;
#[macro_use]
extern crate clap;
extern crate exif;
#[macro_use]
extern crate log;
extern crate rayon;
extern crate simplelog;

pub mod app;
pub mod cli;
pub mod imgfile;
pub mod rename;

use std::path::Path;
use std::process::exit;

use chrono_tz::Tz;

use app::AppBuilder;

fn main() {
    let matches = cli::build_cli().get_matches();

    let mut app_builder = AppBuilder::new();
    app_builder.with_dry_run(matches.is_present("dry_run"));
    app_builder.with_assume_yes(matches.is_present("assume_yes"));
    if matches.is_present("timezone") {
        let timezone = value_t_or_exit!(matches, "timezone", Tz);
        app_builder.with_timezone(timezone);
    };
    if let Some(format) = matches.value_of("format") {
        app_builder.with_name_format(format);
    }
    if matches.is_present("log_level") {
        let log_level = value_t_or_exit!(matches, "log_level", log::LevelFilter);
        app_builder.with_log_level(log_level);
    }

    let app = app_builder.build();
    app.init();

    let source_dir = Path::new(matches.value_of("source").unwrap());
    exit(app.run(source_dir));
}
