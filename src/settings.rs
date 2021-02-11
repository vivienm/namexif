use chrono_tz::Tz;
use clap::{value_t_or_exit, ArgMatches};

pub struct Settings<'a> {
    pub name_format: &'a str,
    pub timezone: Option<Tz>,
    pub log_level: log::LevelFilter,
    pub dry_run: bool,
    pub assume_yes: bool,
}

impl<'a> Settings<'a> {
    pub fn from_matches(matches: &'a ArgMatches) -> Self {
        Settings {
            name_format: matches.value_of("format").unwrap_or("%Y-%m-%dT%H:%M:%S%z"),
            timezone: matches
                .value_of("timezone")
                .map(|_| value_t_or_exit!(matches, "timezone", Tz)),
            log_level: matches
                .value_of("log_level")
                .map(|_| value_t_or_exit!(matches, "log_level", log::LevelFilter))
                .unwrap_or(log::LevelFilter::Info),
            dry_run: matches.is_present("dry_run"),
            assume_yes: matches.is_present("assume_yes"),
        }
    }
}
