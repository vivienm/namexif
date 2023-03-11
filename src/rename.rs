use std::{
    collections::{btree_map, hash_set},
    error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
    result,
};

use chrono::TimeZone;
use derive_more::{Display, From};
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::image;

#[derive(Debug)]
pub enum SkipError {
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

impl error::Error for SkipError {}

#[derive(Debug, Display, From)]
pub enum Error {
    Image(image::Error),
    Skip(SkipError),
}

impl error::Error for Error {}

type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Side {
    Source,
    Target,
}

#[derive(Debug)]
pub struct Conflict<'a> {
    pub side: Side,
    pub path: &'a Path,
}

impl<'a> error::Error for Conflict<'a> {}

impl<'a> fmt::Display for Conflict<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} file {} is overwritten",
            match self.side {
                Side::Source => "Source",
                Side::Target => "Target",
            },
            self.path.display(),
        )
    }
}

pub struct Conflicts<'a> {
    items: btree_map::Iter<'a, PathBuf, Result<PathBuf>>,
    target_paths: hash_set::HashSet<&'a Path>,
}

impl<'a> Iterator for Conflicts<'a> {
    type Item = Conflict<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (source_path, target_path) = self.items.next()?;
            if let Ok(target_path) = target_path {
                let source_path = source_path.as_ref();
                let target_path = target_path.as_ref();
                let conflict = if self.target_paths.contains(source_path) {
                    Some(Conflict {
                        side: Side::Source,
                        path: source_path,
                    })
                } else if self.target_paths.contains(target_path) {
                    Some(Conflict {
                        side: Side::Target,
                        path: target_path,
                    })
                } else {
                    None
                };
                self.target_paths.insert(target_path);
                if conflict.is_some() {
                    return conflict;
                }
            }
        }
    }
}

pub struct Renames {
    items: btree_map::BTreeMap<PathBuf, Result<PathBuf>>,
}

impl Renames {
    pub fn conflicts(&self) -> Conflicts {
        Conflicts {
            items: self.iter(),
            target_paths: hash_set::HashSet::with_capacity(self.items.len()),
        }
    }

    pub fn iter(&self) -> btree_map::Iter<PathBuf, Result<PathBuf>> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

impl IntoIterator for Renames {
    type Item = (PathBuf, Result<PathBuf>);

    type IntoIter = btree_map::IntoIter<PathBuf, Result<PathBuf>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

const JPEG_CANONICAL_EXTENSION: &str = "jpg";
const JPEG_EXTENSIONS: [&str; 4] = [JPEG_CANONICAL_EXTENSION, "JPG", "jpeg", "JPEG"];
const TIFF_CANONICAL_EXTENSION: &str = "tiff";
const TIFF_EXTENSIONS: [&str; 4] = [TIFF_CANONICAL_EXTENSION, "tif", "TIF", "TIFF"];

fn get_target_extension(source_path: &Path) -> Result<&str> {
    let source_extension = source_path
        .extension()
        .and_then(OsStr::to_str)
        .ok_or(Error::Skip(SkipError::Extension))?;
    if JPEG_EXTENSIONS.contains(&source_extension) {
        Ok(JPEG_CANONICAL_EXTENSION)
    } else if TIFF_EXTENSIONS.contains(&source_extension) {
        Ok(TIFF_CANONICAL_EXTENSION)
    } else {
        Err(Error::Skip(SkipError::Extension))
    }
}

fn get_target_file_stem<T>(source_path: &Path, timezone: &T, name_format: &str) -> Result<String>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    if source_path.is_dir() {
        return Err(Error::Skip(SkipError::Directory));
    }
    let image = image::Image::open(source_path)?;
    let datetime = image.get_datetime(timezone)?;
    let file_stem = datetime.format(name_format).to_string();
    Ok(file_stem)
}

fn get_target_name<T>(source_path: &Path, timezone: &T, name_format: &str) -> Result<OsString>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    let target_extension = get_target_extension(source_path)?;
    let target_file_stem = get_target_file_stem(source_path, timezone, name_format)?;
    let mut target_name = target_file_stem;
    target_name.push('.');
    target_name.push_str(target_extension);
    Ok(OsString::from(target_name))
}

fn get_target_path<T>(source_path: &Path, timezone: &T, name_format: &str) -> Result<PathBuf>
where
    T: TimeZone,
    T::Offset: fmt::Display,
{
    let target_name = get_target_name(source_path, timezone, name_format)?;
    let parent_path = source_path.parent().unwrap();
    let target_path = parent_path.join(target_name);
    if source_path == target_path {
        return Err(Error::Skip(SkipError::WellNamed));
    }
    Ok(target_path)
}

fn get_source_paths(source_path: &Path) -> io::Result<Vec<PathBuf>> {
    if source_path.is_file() {
        let source_path = source_path.to_path_buf();
        return Ok(vec![source_path]);
    }
    let read_dir = fs::read_dir(source_path)?;
    let paths: io::Result<Vec<_>> = read_dir
        .map(|result| result.map(|dir_entry| dir_entry.path()))
        .collect();
    let mut paths = paths?;
    paths.sort();
    Ok(paths)
}

pub fn get_renames<T>(source_path: &Path, timezone: &T, name_format: &str) -> io::Result<Renames>
where
    T: TimeZone + Sync,
    T::Offset: fmt::Display,
{
    let source_paths = get_source_paths(source_path)?;
    let items = source_paths.into_par_iter().map(|source_path| {
        let target_path = get_target_path(&source_path, timezone, name_format);
        (source_path, target_path)
    });
    let items = btree_map::BTreeMap::from_par_iter(items);
    Ok(Renames { items })
}
