use std::error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, MAIN_SEPARATOR};
use std::process;
use std::result;

use chrono;
use derive_more::From;
use log;
use simplelog;

use crate::rename;
use crate::settings::Settings;

fn init_logger(settings: &Settings) -> result::Result<(), simplelog::TermLogError> {
    let config = simplelog::Config {
        time: None,
        level: Some(log::Level::Info),
        target: None,
        location: None,
        time_format: None,
    };
    simplelog::TermLogger::init(settings.log_level, config)
}

fn prompt_confirm(
    stdin: &io::Stdin,
    stdout: &mut io::Stdout,
    message: &str,
    default: bool,
) -> io::Result<bool> {
    let mut input = String::new();
    loop {
        print!("{} [{}] ", message, if default { "Yn" } else { "yN" });
        stdout.flush()?;
        stdin.read_line(&mut input)?;
        {
            let input = input.trim_end();
            match input {
                "" => return Ok(default),
                "y" | "Y" => return Ok(true),
                "n" | "N" => return Ok(false),
                _ => eprintln!("Invalid input: {}", input),
            }
        }
        input.clear();
    }
}

pub fn get_renames(source_path: &Path, settings: &Settings) -> io::Result<rename::Renames> {
    match settings.timezone {
        None => rename::get_renames(source_path, &chrono::Local, settings.name_format),
        Some(timezone) => rename::get_renames(source_path, &timezone, settings.name_format),
    }
}

pub fn common_ancestor<'a>(source_path: &'a Path, target_path: &'a Path) -> Option<&'a Path> {
    for ancestor in source_path.ancestors() {
        if target_path.starts_with(ancestor) {
            return Some(ancestor);
        }
    }
    None
}

fn write_rename<W>(f: &mut W, source_path: &Path, target_path: &Path) -> io::Result<()>
where
    W: io::Write,
{
    let mut source_path = source_path;
    let mut target_path = target_path;
    let mut ancestor_empty = true;
    if let Some(ancestor_path) = common_ancestor(source_path, target_path) {
        source_path = source_path.strip_prefix(ancestor_path).unwrap();
        target_path = target_path.strip_prefix(ancestor_path).unwrap();
        for component in ancestor_path.components() {
            if let Component::CurDir = component {
                continue;
            }
            write!(f, "{}", component.as_os_str().to_string_lossy())?;
            ancestor_empty = false;
            match component {
                Component::ParentDir | Component::Normal(_) => {
                    write!(f, "{}", MAIN_SEPARATOR)?;
                }
                _ => {}
            }
        }
    }
    writeln!(
        f,
        "{}{} => {}{}",
        if ancestor_empty { "" } else { "{" },
        source_path.display(),
        target_path.display(),
        if ancestor_empty { "" } else { "}" },
    )?;
    Ok(())
}

#[inline]
fn pluralize(value: usize) -> &'static str {
    if value >= 2 {
        "s"
    } else {
        ""
    }
}

#[derive(Debug, From)]
pub enum Error {
    Io(io::Error),
    TermLog(simplelog::TermLogError),
    Conflicts(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(err) => err.fmt(f),
            Error::TermLog(err) => err.fmt(f),
            Error::Conflicts(n) => write!(f, "{} conflicting file{}", n, pluralize(*n)),
        }
    }
}

impl error::Error for Error {}

type Result<T> = result::Result<T, Error>;

fn try_run(source_path: &Path, settings: &Settings) -> Result<(usize, usize)> {
    init_logger(settings)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let renames = get_renames(source_path, settings)?;

    // Look for errors and retrieve paths.
    let mut paths: Vec<(&Path, &Path)> = Vec::with_capacity(renames.len());
    let mut errors = 0;
    for (source_path, target_path) in renames.iter() {
        match target_path {
            Err(rename::Error::Skip(err)) => {
                log::info!("Skipping file {}: {}", source_path.display(), err);
            }
            Err(rename::Error::Image(err)) => {
                log::error!("Skipping file {}: {}", source_path.display(), err);
                errors += 1;
            }
            Ok(target_path) => {
                paths.push((source_path, target_path));
            }
        }
    }

    // Display paths.
    for (source_path, target_path) in &paths {
        write_rename(&mut stdout, source_path, target_path)?;
    }

    // Look for conflicts.
    let mut conflicts = 0;
    for conflict in renames.conflicts() {
        log::error!("{}", conflict);
        conflicts += 1;
    }
    if conflicts > 0 {
        return Err(Error::Conflicts(conflicts));
    }

    // Rename files.
    let mut renamed = 0;
    if !paths.is_empty()
        && !settings.dry_run
        && (settings.assume_yes || prompt_confirm(&stdin, &mut stdout, "Proceed?", false)?)
    {
        for (source_path, target_path) in &paths {
            match fs::rename(source_path, target_path) {
                Err(err) => {
                    log::error!(
                        "Can't rename {} to {}: {}",
                        source_path.display(),
                        target_path.display(),
                        err
                    );
                    errors += 1;
                }
                Ok(_) => {
                    renamed += 1;
                }
            }
        }
    }
    Ok((renamed, errors))
}

pub fn run(source_path: &Path, settings: &Settings) -> ! {
    match try_run(source_path, settings) {
        Ok((0, 0)) => {
            log::info!("Nothing to do");
            process::exit(0);
        }
        Ok((renamed, 0)) => {
            log::info!("{} renamed file{}", renamed, pluralize(renamed));
            process::exit(0);
        }
        Ok((renamed, errors)) => {
            log::info!(
                "{} renamed file{}, {} error{}",
                renamed,
                pluralize(renamed),
                errors,
                pluralize(errors)
            );
            process::exit(1);
        }
        Err(err) => {
            log::error!("{}", err);
            process::exit(2);
        }
    }
}
