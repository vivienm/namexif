use std::collections::HashSet;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};

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
enum SkipError {
    Directory,
    Extension,
    WellNamed,
}

impl fmt::Display for SkipError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SkipError::Directory => write!(f, "Is a directory"),
            SkipError::Extension => write!(f, "Not an EXIF file"),
            SkipError::WellNamed => write!(f, "Does not need renaming"),
        }
    }
}

impl Error for SkipError {}

#[derive(Debug)]
enum DateError {
    InvalidLocalDatetime,
    AmbiguousLocalDatetime,
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DateError::InvalidLocalDatetime => write!(f, "Invalid local date"),
            DateError::AmbiguousLocalDatetime => write!(f, "Ambiguous local date"),
        }
    }
}

impl Error for DateError {}

#[derive(Debug)]
enum RenameError {
    Image(ImageError),
    Skip(SkipError),
    Date(DateError),
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RenameError::Image(err) => err.fmt(f),
            RenameError::Skip(err) => err.fmt(f),
            RenameError::Date(err) => err.fmt(f),
        }
    }
}

impl Error for RenameError {}

impl RenameError {
    fn should_raise(&self) -> bool {
        match self {
            RenameError::Image(..) | RenameError::Date(..) => true,
            RenameError::Skip(..) => false,
        }
    }

    fn log_level(&self) -> log::Level {
        if self.should_raise() {
            log::Level::Error
        } else {
            log::Level::Info
        }
    }

    fn log(&self, path: &Path) {
        let level = self.log_level();
        log!(level, "Skipping {}: {}", path.display(), self);
    }
}

type RenameResult<T> = Result<T, RenameError>;

fn get_target_extension(source_path: &Path) -> RenameResult<&str> {
    let source_extension = source_path
        .extension()
        .and_then(OsStr::to_str)
        .ok_or_else(|| RenameError::Skip(SkipError::Extension))?;
    let target_extension = if JPEG_EXTENSIONS.contains(&source_extension) {
        JPEG_CANONICAL_EXTENSION
    } else if TIFF_EXTENSIONS.contains(&source_extension) {
        TIFF_CANONICAL_EXTENSION
    } else {
        return Err(RenameError::Skip(SkipError::Extension));
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
    let image = ImageFile::open(source_path).map_err(RenameError::Image)?;
    let naive_datetime = image.get_naive_datetime().map_err(RenameError::Image)?;
    let datetime = match timezone.from_local_datetime(&naive_datetime) {
        LocalResult::None => {
            return Err(RenameError::Date(DateError::InvalidLocalDatetime));
        }
        LocalResult::Single(datetime) => datetime,
        LocalResult::Ambiguous(..) => {
            return Err(RenameError::Date(DateError::AmbiguousLocalDatetime));
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
        return Err(RenameError::Skip(SkipError::Directory));
    }
    let target_name = get_target_name(source_path, timezone, name_format)?;
    let parent_path = source_path.parent().unwrap();
    let target_path = parent_path.join(target_name);
    if source_path == target_path {
        return Err(RenameError::Skip(SkipError::WellNamed));
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

    pub fn common_ancestor(&self) -> Option<&Path> {
        for ancestor in self.source_path.ancestors() {
            if self.target_path.starts_with(ancestor) {
                return Some(ancestor);
            }
        }
        None
    }
}

impl fmt::Display for RenameItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut source_path = self.source_path.as_path();
        let mut target_path = self.target_path.as_path();
        let mut ancestor_empty = true;
        if let Some(ancestor_path) = self.common_ancestor() {
            source_path = self.source_path.strip_prefix(ancestor_path).unwrap();
            target_path = self.target_path.strip_prefix(ancestor_path).unwrap();
            for component in ancestor_path.components() {
                match component {
                    Component::CurDir => {
                        continue;
                    }
                    _ => {}
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
        write!(
            f,
            "{}{} => {}{}",
            if ancestor_empty { "" } else { "{" },
            source_path.display(),
            target_path.display(),
            if ancestor_empty { "" } else { "}" },
        )?;
        Ok(())
    }
}

#[test]
fn test_rename_item_fmt() {
    fn formatted(source_path: &str, target_path: &str) -> String {
        let source_path = Path::new(source_path).to_path_buf();
        let target_path = Path::new(target_path).to_path_buf();
        let rename_item = RenameItem::new(source_path, target_path);
        format!("{}", rename_item)
    }

    assert_eq!(formatted("foo", "bar"), "foo => bar");
    assert_eq!(formatted("/foo", "/bar"), "/{foo => bar}");
    assert_eq!(formatted("./foo", "./bar"), "foo => bar");
    assert_eq!(
        formatted("path/to/foo", "path/to/bar"),
        "path/to/{foo => bar}"
    );
    assert_eq!(
        formatted("/path/to/foo", "/path/to/bar"),
        "/path/to/{foo => bar}"
    );
    assert_eq!(
        formatted("./path/to/foo", "./path/to/bar"),
        "path/to/{foo => bar}"
    );
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
            }).collect();
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
                Err(true) => {
                    self.should_raise = true;
                    None
                }
                Err(false) => None,
            }).collect();
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

    fn check_items(&mut self, items: &[RenameItem]) -> Result<(), ()> {
        let mut target_paths = HashSet::with_capacity(items.len());
        let mut should_raise = false;
        for item in items {
            if target_paths.contains(&item.source_path) {
                error!("Source file {} is overwritten", item.source_path.display());
                should_raise = true;
            } else if !target_paths.insert(&item.target_path) {
                error!("Target file {} is overwritten", item.target_path.display());
                should_raise = true;
            }
        }
        if should_raise {
            self.should_raise = true;
            Err(())
        } else {
            Ok(())
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
        if self.check_items(&items).is_ok()
            && !self.settings.dry_run
            && (self.settings.assume_yes || prompt_confirm("Proceed?", false).unwrap())
        {
            self.apply_items(&items);
        }
        if self.should_raise {
            return Err(());
        }
        Ok(())
    }
}
