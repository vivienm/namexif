use std::{
    collections::{btree_map, hash_set},
    error,
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
    result,
};

use derive_more::{Display, From};
use jiff::tz;
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
            SkipError::Extension => write!(f, "Unsupported file format"),
            SkipError::WellNamed => write!(f, "Does not need renaming"),
        }
    }
}

impl error::Error for SkipError {}

#[derive(Debug, Display, From)]
pub enum Error {
    Image(image::Error),
    Skip(SkipError),
    #[display("Source path has no parent directory")]
    NoParent,
}

impl error::Error for Error {}

type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Side {
    Source,
    Target,
    Existing,
}

#[derive(Debug)]
pub struct Conflict<'a> {
    pub side: Side,
    pub path: &'a Path,
}

impl error::Error for Conflict<'_> {}

impl fmt::Display for Conflict<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} file {} would be overwritten",
            match self.side {
                Side::Source => "Source",
                Side::Target => "Target",
                Side::Existing => "Existing",
            },
            self.path.display(),
        )
    }
}

pub struct Conflicts<'a> {
    items: btree_map::Iter<'a, PathBuf, Result<PathBuf>>,
    source_paths: &'a btree_map::BTreeMap<PathBuf, Result<PathBuf>>,
    target_paths: hash_set::HashSet<&'a Path>,
}

impl<'a> Iterator for Conflicts<'a> {
    type Item = Conflict<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (source_path, target_path) = self.items.next()?;
            if let Ok(target_path) = target_path {
                let source_path = source_path.as_ref();
                let target_path: &Path = target_path.as_ref();
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
                } else if !self.source_paths.contains_key(target_path)
                    && target_path.try_exists().unwrap_or(false)
                    && !same_file::is_same_file(source_path, target_path).unwrap_or(false)
                {
                    Some(Conflict {
                        side: Side::Existing,
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
    pub fn conflicts(&self) -> Conflicts<'_> {
        Conflicts {
            items: self.iter(),
            source_paths: &self.items,
            target_paths: hash_set::HashSet::with_capacity(self.items.len()),
        }
    }

    pub fn iter(&self) -> btree_map::Iter<'_, PathBuf, Result<PathBuf>> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
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
    if source_path.is_dir() {
        return Err(Error::Skip(SkipError::Directory));
    }
    let image = image::Image::open(source_path)?;
    let zoned = image.get_zoned(timezone)?;
    let file_stem = zoned.strftime(name_format).to_string();
    Ok(file_stem)
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

pub fn get_renames(
    source_path: &Path,
    timezone: &tz::TimeZone,
    name_format: &str,
) -> io::Result<Renames> {
    let source_paths = get_source_paths(source_path)?;
    let items = source_paths.into_par_iter().map(|source_path| {
        let target_path = get_target_path(&source_path, timezone, name_format);
        (source_path, target_path)
    });
    let items = btree_map::BTreeMap::from_par_iter(items);
    Ok(Renames { items })
}
