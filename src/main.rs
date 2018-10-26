extern crate chrono;
extern crate chrono_tz;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate derive_more;
extern crate exif;
#[macro_use]
extern crate log;
extern crate rayon;
extern crate simplelog;

mod app;
mod image;
mod rename;
mod settings;
mod ui;

use std::path::Path;

use app::build_app;
use settings::Settings;

fn main() {
    let matches = build_app().get_matches();
    let settings = Settings::from_matches(&matches);
    let source_path = Path::new(matches.value_of("source").unwrap_or("."));
    ui::run(source_path, &settings);
}
