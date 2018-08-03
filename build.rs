#[macro_use]
extern crate clap;

use std::env;
use std::fs;

use clap::Shell;

#[allow(dead_code)]
#[path = "src/cli.rs"]
mod cli;

fn main() {
    let mut app = cli::build_cli();
    let output_dir = env::var_os("OUT_DIR").unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    app.gen_completions("namexif", Shell::Bash, &output_dir);
    app.gen_completions("namexif", Shell::Zsh, &output_dir);
    app.gen_completions("namexif", Shell::Fish, &output_dir);
}
