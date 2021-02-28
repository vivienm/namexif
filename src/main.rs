mod app;
mod cli;
mod image;
mod rename;

use structopt::StructOpt;

fn main() {
    app::main(&cli::Args::from_args())
}
