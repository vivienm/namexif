use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::offset::LocalResult;
use chrono::{Local, TimeZone};
use log;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use image::{ImageError, ImageFile};
use settings::Settings;
use utils::prompt_confirm;

const JPEG_CANONICAL_EXTENSION: &str = "jpg";
const JPEG_EXTENSIONS: [&str; 4] = [JPEG_CANONICAL_EXTENSION, "JPG", "jpeg", "JPEG"];
const TIFF_CANONICAL_EXTENSION: &str = "tiff";
const TIFF_EXTENSIONS: [&str; 4] = [TIFF_CANONICAL_EXTENSION, "tif", "TIF", "TIFF"];

#[derive(Debug)]
enum RenameError {
    IsADirectory,
    MissingExtension,
    InvalidExtension,
    ImageError(ImageError),
    InvalidLocalDatetime,
    AmbiguousLocalDatetime,
    IdenticalNames,
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RenameError::IsADirectory => write!(f, "Is a directory"),
            RenameError::MissingExtension => write!(f, "Missing extension"),
            RenameError::InvalidExtension => write!(f, "Invalid extension"),
            RenameError::ImageError(err) => err.fmt(f),
            RenameError::InvalidLocalDatetime => write!(f, "Invalid local time representation"),
            RenameError::AmbiguousLocalDatetime => write!(f, "Ambiguous local time representation"),
            RenameError::IdenticalNames => write!(f, "Already well named"),
        }
    }
}

impl Error for RenameError {}

impl RenameError {
    fn log_level(&self) -> log::Level {
        match self {
            RenameError::IsADirectory => log::Level::Info,
            RenameError::MissingExtension => log::Level::Info,
            RenameError::InvalidExtension => log::Level::Info,
            RenameError::ImageError(..) => log::Level::Error,
            RenameError::InvalidLocalDatetime => log::Level::Error,
            RenameError::AmbiguousLocalDatetime => log::Level::Error,
            RenameError::IdenticalNames => log::Level::Info,
        }
    }

    fn log(&self, path: &Path) {
        let level = self.log_level();
        log!(level, "Skipping {}: {}", path.display(), self);
    }

    fn should_raise(&self) -> bool {
        self.log_level() > log::Level::Info
    }
}

type RenameResult<T> = Result<T, RenameError>;

fn get_target_extension(source_path: &Path) -> RenameResult<&str> {
    let source_extension = match source_path.extension().and_then(OsStr::to_str) {
        None => {
            return Err(RenameError::MissingExtension);
        }
        Some(source_extension) => source_extension,
    };
    let target_extension = if JPEG_EXTENSIONS.contains(&source_extension) {
        JPEG_CANONICAL_EXTENSION
    } else if TIFF_EXTENSIONS.contains(&source_extension) {
        TIFF_CANONICAL_EXTENSION
    } else {
        return Err(RenameError::InvalidExtension);
    };
    Ok(target_extension)
}

fn get_target_file_stem<T>(
    source_path: &Path,
    timezone: &T,
    name_format: &str,
) -> RenameResult<String>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    let image = ImageFile::open(source_path).map_err(RenameError::ImageError)?;
    let naive_datetime = image.get_datetime().map_err(RenameError::ImageError)?;
    let datetime = match timezone.from_local_datetime(&naive_datetime) {
        LocalResult::None => {
            return Err(RenameError::InvalidLocalDatetime);
        }
        LocalResult::Single(datetime) => datetime,
        LocalResult::Ambiguous(..) => {
            return Err(RenameError::AmbiguousLocalDatetime);
        }
    };
    let file_stem = datetime.format(name_format).to_string();
    Ok(file_stem)
}

fn get_target_name<T>(source_path: &Path, timezone: &T, name_format: &str) -> RenameResult<String>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    let target_extension = get_target_extension(source_path)?;
    let target_file_stem = get_target_file_stem(source_path, timezone, name_format)?;
    let mut target_name = target_file_stem;
    target_name.push('.');
    target_name.push_str(target_extension);
    Ok(target_name)
}

fn get_target_path<T>(source_path: &Path, timezone: &T, name_format: &str) -> RenameResult<PathBuf>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    if source_path.is_dir() {
        return Err(RenameError::IsADirectory);
    }
    let target_name = get_target_name(source_path, timezone, name_format)?;
    let parent_path = source_path.parent().unwrap();
    let target_path = parent_path.join(target_name);
    if source_path == target_path {
        return Err(RenameError::IdenticalNames);
    }
    Ok(target_path)
}

pub struct RenameItem {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
}

impl RenameItem {
    pub fn new(source_path: PathBuf, target_path: PathBuf) -> Self {
        Self {
            source_path,
            target_path,
        }
    }

    pub fn apply(&self) -> io::Result<()> {
        fs::rename(&self.source_path, &self.target_path)
    }
}

impl fmt::Display for RenameItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} => {}",
            self.source_path.display(),
            self.target_path.display(),
        )
    }
}

pub struct BatchRenamer<'a> {
    settings: &'a Settings<'a>,
    should_raise: bool,
}

impl<'a> BatchRenamer<'a> {
    pub fn new(settings: &'a Settings<'a>) -> Self {
        Self {
            settings,
            should_raise: false,
        }
    }

    fn get_source_paths(&mut self, source_dir: &Path) -> Vec<PathBuf> {
        if source_dir.is_file() {
            let source_path = source_dir.to_path_buf();
            return vec![source_path];
        }
        let direntries = match fs::read_dir(source_dir) {
            Ok(direntries) => direntries,
            Err(err) => {
                error!("Skipping directory {}: {}", source_dir.display(), err);
                self.should_raise = true;
                return vec![];
            }
        };
        let mut paths: Vec<_> = direntries
            .filter_map(|direntry| match direntry {
                Ok(direntry) => Some(direntry.path()),
                Err(err) => {
                    error!("Skipping file: {}", err);
                    self.should_raise = true;
                    None
                }
            })
            .collect();
        paths.sort();
        paths
    }

    fn get_item(&self, source_path: PathBuf) -> Result<RenameItem, bool> {
        let target_path = match self.settings.timezone {
            None => get_target_path(&source_path, &Local, self.settings.name_format),
            Some(timezone) => get_target_path(&source_path, &timezone, self.settings.name_format),
        };
        match target_path {
            Ok(target_path) => {
                let item = RenameItem::new(source_path, target_path);
                Ok(item)
            }
            Err(err) => {
                err.log(&source_path);
                Err(err.should_raise())
            }
        }
    }

    fn get_items(&mut self, source_dir: &Path) -> Vec<RenameItem> {
        let source_paths = self.get_source_paths(source_dir);
        let results: Vec<_> = source_paths
            .into_par_iter()
            .map(|source_path| self.get_item(source_path))
            .collect();
        let items: Vec<_> = results
            .into_iter()
            .filter_map(|result| match result {
                Ok(item) => Some(item),
                Err(should_raise) => {
                    if should_raise {
                        self.should_raise = true
                    };
                    None
                }
            })
            .collect();
        items
    }

    fn apply_item(&mut self, item: &RenameItem) {
        match item.apply() {
            Ok(_) => (),
            Err(err) => {
                error!(
                    "Can't rename file {} to {}: {}",
                    item.source_path.display(),
                    item.target_path.display(),
                    err
                );
                self.should_raise = true;
            }
        }
    }

    fn apply_items(&mut self, items: &[RenameItem]) {
        for item in items {
            self.apply_item(item);
        }
    }

    pub fn run(mut self, source_dir: &Path) -> Result<(), ()> {
        let items = self.get_items(source_dir);
        if self.should_raise {
            return Err(());
        } else if items.is_empty() {
            info!("Nothing to do");
            return Ok(());
        };
        for item in &items {
            println!("{}", item);
        }
        if !self.settings.dry_run
            && (self.settings.assume_yes || prompt_confirm("OK?", false).unwrap())
        {
            self.apply_items(&items);
        }
        if self.should_raise {
            return Err(());
        }
        Ok(())
    }
}
