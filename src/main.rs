mod app;
mod image;
mod rename;
mod settings;
mod ui;

use std::path::Path;

use crate::app::build_app;
use crate::settings::Settings;

fn main() {
    let matches = build_app().get_matches();
    let settings = Settings::from_matches(&matches);
    let source_path = Path::new(matches.value_of("source").unwrap_or("."));
    ui::run(source_path, &settings);
}
