use clap::Parser;

mod app;
mod cli;
mod image;
mod rename;

fn main() {
    app::main(&cli::Args::parse())
}
