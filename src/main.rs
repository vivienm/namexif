extern crate chrono;
extern crate chrono_tz;
#[macro_use]
extern crate clap;
extern crate exif;
#[macro_use]
extern crate log;
extern crate rayon;
extern crate simplelog;

mod app;
mod image;
mod rename;
mod settings;
mod utils;

use std::path::Path;
use std::process::exit;

use app::build_app;
use rename::BatchRenamer;
use settings::Settings;

fn init_logger(settings: &Settings) -> Result<(), simplelog::TermLogError> {
    let config = simplelog::Config {
        time: None,
        level: Some(log::Level::Info),
        target: None,
        location: None,
        time_format: None,
    };
    simplelog::TermLogger::init(settings.log_level, config)
}

fn main() {
    let matches = build_app().get_matches();
    let settings = Settings::from_matches(&matches);
    let source_dir = Path::new(matches.value_of("source").unwrap_or("."));
    init_logger(&settings).unwrap();
    let renamer = BatchRenamer::new(&settings);
    match renamer.run(source_dir) {
        Err(()) => exit(2),
        Ok(()) => (),
    }
}
