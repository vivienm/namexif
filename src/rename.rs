use std::{
    error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    result,
};

use derive_more::{Display, From};
use jiff::tz;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::image;

#[derive(Debug)]
pub enum SkipError {
    Directory,
    NotRegularFile,
    Extension,
    WellNamed,
}

impl fmt::Display for SkipError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SkipError::Directory => write!(f, "Is a directory"),
            SkipError::NotRegularFile => write!(f, "Is not a regular file"),
            SkipError::Extension => write!(f, "Unsupported file format"),
            SkipError::WellNamed => write!(f, "Does not need renaming"),
        }
    }
}

impl error::Error for SkipError {}

#[derive(Debug, Display, From)]
pub enum Error {
    Io(io::Error),
    Image(image::Error),
    #[display("Invalid filename format: {_0}")]
    Format(jiff::Error),
    #[display("Invalid filename format: result must be a single filename")]
    InvalidFilename,
    Skip(SkipError),
    #[display("Source path has no parent directory")]
    NoParent,
}

impl error::Error for Error {}

type Result<T> = result::Result<T, Error>;

const JPEG_CANONICAL_EXTENSION: &str = "jpg";
const TIFF_CANONICAL_EXTENSION: &str = "tiff";

fn get_target_extension(source_path: &Path) -> Result<&str> {
    let ext = source_path
        .extension()
        .and_then(OsStr::to_str)
        .ok_or(Error::Skip(SkipError::Extension))?;
    if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        Ok(JPEG_CANONICAL_EXTENSION)
    } else if ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff") {
        Ok(TIFF_CANONICAL_EXTENSION)
    } else {
        Err(Error::Skip(SkipError::Extension))
    }
}

fn get_target_file_stem(
    source_path: &Path,
    timezone: &tz::TimeZone,
    name_format: &str,
) -> Result<String> {
    // Follow symlinks to classify their referents, just as Image::open does.
    // Unlike Path::is_file/is_dir, metadata preserves errors for the caller.
    let metadata = fs::metadata(source_path)?;
    if metadata.is_dir() {
        return Err(Error::Skip(SkipError::Directory));
    }
    if !metadata.is_file() {
        return Err(Error::Skip(SkipError::NotRegularFile));
    }
    let image = image::Image::open(source_path)?;
    let zoned = image.get_zoned(timezone)?;
    jiff::fmt::strtime::format(name_format, &zoned).map_err(Error::Format)
}

fn get_target_name(
    source_path: &Path,
    timezone: &tz::TimeZone,
    name_format: &str,
) -> Result<OsString> {
    let target_extension = get_target_extension(source_path)?;
    let target_file_stem = get_target_file_stem(source_path, timezone, name_format)?;
    let mut target_name = target_file_stem;
    target_name.push('.');
    target_name.push_str(target_extension);
    let mut components = Path::new(&target_name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(Error::InvalidFilename);
    }
    Ok(OsString::from(target_name))
}

fn get_target_path(
    source_path: &Path,
    timezone: &tz::TimeZone,
    name_format: &str,
) -> Result<PathBuf> {
    let target_name = get_target_name(source_path, timezone, name_format)?;
    let parent_path = source_path.parent().ok_or(Error::NoParent)?;
    let target_path = parent_path.join(target_name);
    if source_path == target_path {
        return Err(Error::Skip(SkipError::WellNamed));
    }
    Ok(target_path)
}

fn get_source_paths(source_path: &Path) -> io::Result<Vec<PathBuf>> {
    if !fs::metadata(source_path)?.is_dir() {
        return Ok(vec![source_path.to_path_buf()]);
    }
    let read_dir = fs::read_dir(source_path)?;
    read_dir
        .map(|result| result.map(|dir_entry| dir_entry.path()))
        .collect()
}

pub fn get_renames(
    source_path: &Path,
    timezone: &tz::TimeZone,
    name_format: &str,
) -> io::Result<Vec<(PathBuf, Result<PathBuf>)>> {
    let source_paths = get_source_paths(source_path)?;
    Ok(source_paths
        .into_par_iter()
        .map(|source_path| {
            let target_path = get_target_path(&source_path, timezone, name_format);
            (source_path, target_path)
        })
        .collect())
}
