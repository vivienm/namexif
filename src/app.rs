use clap::{App, Arg};

pub fn build_app() -> App<'static, 'static> {
    app_from_crate!()
        .arg(
            Arg::with_name("timezone")
                .short("z")
                .long("timezone")
                .env("NAMEXIF_TIMEZONE")
                .takes_value(true)
                .value_name("TIMEZONE")
                .help("Time zone"),
        ).arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .env("NAMEXIF_FORMAT")
                .takes_value(true)
                .value_name("FORMAT")
                .help("Filename format"),
        ).arg(
            Arg::with_name("dry_run")
                .short("n")
                .long("dry-run")
                .help("Does not actually rename files"),
        ).arg(
            Arg::with_name("assume_yes")
                .short("y")
                .long("assume-yes")
                .help("Does not prompt for confirmation"),
        ).arg(
            Arg::with_name("log_level")
                .short("l")
                .long("log-level")
                .env("NAMEXIF_LOG_LEVEL")
                .takes_value(true)
                .value_name("LEVEL")
                .help("Log verbosity level"),
        ).arg(
            Arg::with_name("source")
                .value_name("SOURCE")
                .index(1)
                .help("Input file or directory"),
        )
}
