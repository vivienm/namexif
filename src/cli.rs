use clap::{App, Arg};

pub fn build_cli() -> App<'static, 'static> {
    app_from_crate!()
        .arg(
            Arg::with_name("timezone")
                .short("z")
                .long("timezone")
                .env("NAMEXIF_TIMEZONE")
                .takes_value(true)
                .value_name("TIMEZONE")
                .help("Time zone"),
        )
        .arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .env("NAMEXIF_FORMAT")
                .takes_value(true)
                .value_name("FORMAT")
                .help("Filename format"),
        )
        .arg(
            Arg::with_name("dry_run")
                .short("n")
                .long("dry-run")
                .help("Do not actually rename files"),
        )
        .arg(
            Arg::with_name("assume_yes")
                .short("y")
                .long("assume-yes")
                .help("Do not prompt for confirmation"),
        )
        .arg(
            Arg::with_name("log_level")
                .short("l")
                .long("log-level")
                .env("NAMEXIF_LOG_LEVEL")
                .takes_value(true)
                .value_name("LOG_LEVEL")
                .help("Log verbosity level"),
        )
        .arg(
            Arg::with_name("source")
                .value_name("SOURCE")
                .required(true)
                .index(1)
                .help("Input image file"),
        )
}
