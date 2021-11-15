use structopt::StructOpt;

mod app;
mod cli;
mod image;
mod rename;

fn main() {
    app::main(&cli::Args::from_args())
}
