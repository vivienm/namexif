#[macro_use]
extern crate clap;

use std::env;
use std::fs;

use clap::Shell;

#[allow(dead_code)]
#[path = "src/app.rs"]
mod app;

fn main() {
    let mut app = app::build_app();
    let output_dir = env::var_os("OUT_DIR").unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    app.gen_completions("namexif", Shell::Bash, &output_dir);
    app.gen_completions("namexif", Shell::Zsh, &output_dir);
    app.gen_completions("namexif", Shell::Fish, &output_dir);
}
