mod image;
mod rename;

use std::{
    fmt, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf, MAIN_SEPARATOR},
    process, result,
};

use chrono_tz::Tz;
use derive_more::{Error, From};

#[derive(Debug, clap::Parser)]
#[clap(about)]
pub struct Args {
    /// Does not prompt for confirmation
    #[arg(short = 'y', long = "assume-yes")]
    pub assume_yes: bool,
    /// Does not actually rename files
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,
    /// Filename format
    #[arg(
        short = 'f',
        long = "format",
        value_name = "format",
        env = "NAMEXIF_FORMAT",
        default_value = "%Y-%m-%dT%H:%M:%S%z"
    )]
    pub name_format: String,
    /// Time zone
    #[arg(short = 'z', long = "timezone", env = "NAMEXIF_TIMEZONE")]
    pub timezone: Option<Tz>,
    /// Generate the completion script for the specified shell.
    #[arg(long, exclusive = true, name = "SHELL")]
    completion: Option<clap_complete::Shell>,
    /// Input file or directory
    #[arg(value_name = "input", default_value = ".")]
    pub source_path: PathBuf,
    /// Set the verbosity level for log messages.
    #[arg(global = true, long, default_value = "info", env = "NAMEXIF_LOG_LEVEL")]
    log_level: tracing::level_filters::LevelFilter,
}

#[inline]
fn pluralize(value: usize) -> &'static str {
    if value >= 2 {
        "s"
    } else {
        ""
    }
}

#[derive(Debug, From, Error)]
pub enum Error {
    Io(io::Error),
    #[error(ignore)]
    Conflicts(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(err) => err.fmt(f),
            Error::Conflicts(n) => write!(f, "{} conflicting file{}", n, pluralize(*n)),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

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

pub fn get_renames(args: &Args) -> io::Result<rename::Renames> {
    match args.timezone {
        None => rename::get_renames(&args.source_path, &chrono::Local, &args.name_format),
        Some(timezone) => rename::get_renames(&args.source_path, &timezone, &args.name_format),
    }
}

pub fn common_ancestor<'a>(source_path: &'a Path, target_path: &'a Path) -> Option<&'a Path> {
    source_path
        .ancestors()
        .find(|&ancestor| target_path.starts_with(ancestor))
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

fn try_run(args: &Args) -> Result<(usize, usize)> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let renames = get_renames(args)?;

    // Look for errors and retrieve paths.
    let mut paths: Vec<(&Path, &Path)> = Vec::with_capacity(renames.len());
    let mut errors = 0;
    for (source_path, target_path) in renames.iter() {
        match target_path {
            Err(rename::Error::Skip(err)) => {
                tracing::info!("Skipping file {}: {}", source_path.display(), err);
            }
            Err(rename::Error::Image(err)) => {
                tracing::error!("Skipping file {}: {}", source_path.display(), err);
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
        tracing::error!("{}", conflict);
        conflicts += 1;
    }
    if conflicts > 0 {
        return Err(Error::Conflicts(conflicts));
    }

    // Rename files.
    let mut renamed = 0;
    if !paths.is_empty()
        && !args.dry_run
        && (args.assume_yes || prompt_confirm(&stdin, &mut stdout, "Proceed?", false)?)
    {
        for (source_path, target_path) in &paths {
            match fs::rename(source_path, target_path) {
                Err(err) => {
                    tracing::error!(
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

fn generate_completions(shell: clap_complete::Shell) -> ! {
    clap_complete::generate(
        shell,
        &mut <Args as clap::CommandFactory>::command(),
        clap::crate_name!(),
        &mut std::io::stdout(),
    );
    std::process::exit(0);
}

fn setup_logging(log_level: tracing::level_filters::LevelFilter) -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::Parser>::parse();
    if let Some(shell) = args.completion {
        generate_completions(shell);
    }
    setup_logging(args.log_level)?;

    match try_run(&args) {
        Ok((0, 0)) => {
            tracing::info!("Nothing to do");
            process::exit(0);
        }
        Ok((renamed, 0)) => {
            tracing::info!("{} renamed file{}", renamed, pluralize(renamed));
            process::exit(0);
        }
        Ok((renamed, errors)) => {
            tracing::info!(
                "{} renamed file{}, {} error{}",
                renamed,
                pluralize(renamed),
                errors,
                pluralize(errors)
            );
            process::exit(1);
        }
        Err(err) => {
            tracing::error!("{}", err);
            process::exit(2);
        }
    }
}
