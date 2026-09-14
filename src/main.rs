mod image;
mod rename;

use std::{
    fmt,
    io::{self, Write},
    path::PathBuf,
    process, result,
};

use derive_more::{Error, From};
use jiff::tz;

#[cfg(windows)]
const DEFAULT_NAME_FORMAT: &str = "%Y-%m-%dT%H%M%S%z";
#[cfg(not(windows))]
const DEFAULT_NAME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%z";

fn parse_timezone(s: &str) -> result::Result<tz::TimeZone, String> {
    tz::TimeZone::get(s).map_err(|err| err.to_string())
}

#[derive(Debug, clap::Parser)]
#[clap(about)]
struct Args {
    /// Does not prompt for confirmation
    #[arg(short = 'y', long = "assume-yes")]
    assume_yes: bool,
    /// Does not actually rename files
    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,
    /// Filename format
    #[arg(
        short = 'f',
        long = "format",
        value_name = "format",
        env = "NAMEXIF_FORMAT",
        default_value = DEFAULT_NAME_FORMAT
    )]
    name_format: String,
    /// Time zone
    #[arg(
        short = 'z',
        long = "timezone",
        env = "NAMEXIF_TIMEZONE",
        value_parser = parse_timezone,
    )]
    timezone: Option<tz::TimeZone>,
    /// Generate the completion script for the specified shell.
    #[arg(long, exclusive = true, name = "SHELL")]
    completion: Option<clap_complete::Shell>,
    /// Input file or directory
    #[arg(value_name = "input", default_value = ".")]
    source_path: PathBuf,
    /// Set the verbosity level for log messages.
    #[arg(global = true, long, default_value = "info", env = "NAMEXIF_LOG_LEVEL")]
    log_level: tracing::level_filters::LevelFilter,
}

#[inline]
fn pluralize(value: usize) -> &'static str {
    if value == 1 { "" } else { "s" }
}

#[derive(Debug, From, Error)]
enum Error {
    Io(io::Error),
    Plan(nominal::PlanError),
    #[error(ignore)]
    Conflicts(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(err) => err.fmt(f),
            Error::Plan(err) => err.fmt(f),
            Error::Conflicts(n) => write!(f, "{} conflicting file{}", n, pluralize(*n)),
        }
    }
}

type Result<T> = result::Result<T, Error>;

fn try_run(args: &Args) -> Result<(usize, usize)> {
    let timezone = args.timezone.clone().unwrap_or_else(tz::TimeZone::system);
    let renames = rename::get_renames(&args.source_path, &timezone, &args.name_format)?;

    // Filter classified errors out before handing the plan to nominal: skips
    // are info, derivation errors are real failures we count.
    let mut errors = 0;
    let pairs = renames
        .into_iter()
        .filter_map(|(source_path, target_path)| match target_path {
            Err(rename::Error::Skip(err)) => {
                tracing::info!("Skipping file {}: {}", source_path.display(), err);
                None
            }
            Err(err) => {
                tracing::error!("Skipping file {}: {}", source_path.display(), err);
                errors += 1;
                None
            }
            Ok(target_path) => Some((source_path, target_path)),
        });

    let mut plan = nominal::Renamer::from_iter(pairs).plan()?;

    // Surface targets that already exist on disk outside the batch. They are
    // drained from the plan, so apply only sees safe renames.
    let conflicts = plan.check_fs()?;
    for conflict in &conflicts {
        tracing::error!("{}", conflict);
    }
    if !conflicts.is_empty() {
        return Err(Error::Conflicts(conflicts.len()));
    }

    if plan.is_empty() {
        return Ok((0, errors));
    }

    let ls_colors = lscolors::LsColors::from_env().unwrap_or_default();
    let mut stdout = io::stdout();
    plan.write_colored_to(&mut stdout, &ls_colors)?;
    stdout.flush()?;

    if args.dry_run {
        return Ok((0, errors));
    }
    if !args.assume_yes && plan.confirm()? != Some(true) {
        return Ok((0, errors));
    }

    let mut renamed = 0;
    for result in plan.apply_iter() {
        match result {
            Ok(_) => renamed += 1,
            Err(err) => {
                tracing::error!("{}", err);
                errors += 1;
            }
        }
    }
    Ok((renamed, errors))
}

fn generate_completions(shell: clap_complete::Shell) {
    clap_complete::generate(
        shell,
        &mut <Args as clap::CommandFactory>::command(),
        clap::crate_name!(),
        &mut std::io::stdout(),
    );
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
        return Ok(());
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
