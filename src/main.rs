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
use rename::Renamer;
use settings::Settings;

fn main() {
    let matches = build_app().get_matches();
    let settings = Settings::from_matches(&matches);
    let source_dir = Path::new(matches.value_of("source").unwrap());

    simplelog::TermLogger::init(settings.log_level, simplelog::Config::default()).unwrap();

    let renamer = Renamer::new(&settings);
    exit(renamer.run(source_dir));
}
