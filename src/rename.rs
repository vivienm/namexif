use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};
use log;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use image::ImageFile;
use settings::Settings;

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

enum RenameSkip {
    Extension,
    Directory,
    CantGetDate,
    WellNamed,
}

pub struct Renamer<'a> {
    settings: &'a Settings<'a>,
}

impl<'a> Renamer<'a> {
    const JPEG_EXTENSION: &'static str = "jpg";
    const JPEG_EXTENSIONS: [&'static str; 4] = [Self::JPEG_EXTENSION, "JPG", "jpeg", "JPEG"];
    const TIFF_EXTENSION: &'static str = "tiff";
    const TIFF_EXTENSIONS: [&'static str; 4] = [Self::TIFF_EXTENSION, "tif", "TIF", "TIFF"];

    pub fn new(settings: &'a Settings<'a>) -> Self {
        Renamer {settings}
    }

    fn get_target_extension(&self, source_path: &Path) -> Result<&str, RenameSkip> {
        let source_extension = source_path
            .extension()
            .and_then(OsStr::to_str)
            .ok_or(RenameSkip::Extension)?;
        if Self::JPEG_EXTENSIONS.contains(&source_extension) {
            Ok(Self::JPEG_EXTENSION)
        } else if Self::TIFF_EXTENSIONS.contains(&source_extension) {
            Ok(Self::TIFF_EXTENSION)
        } else {
            Err(RenameSkip::Extension)
        }
    }

    fn get_target_file_stem<T>(
        &self,
        naive_datetime: &NaiveDateTime,
        timezone: T,
    ) -> Result<String, RenameSkip>
    where
        T: TimeZone,
        T::Offset: fmt::Display,
    {
        let datetime = timezone
            .from_local_datetime(naive_datetime)
            .single()
            .ok_or(RenameSkip::CantGetDate)?;
        Ok(datetime.format(self.settings.name_format).to_string())
    }

    fn get_target_name(&self, source_path: &Path) -> Result<String, RenameSkip> {
        let target_extension = self.get_target_extension(&source_path)?;
        let naive_datetime = ImageFile::open(source_path)
            .and_then(|imgfile| imgfile.get_datetime())
            .map_err(|_| RenameSkip::CantGetDate)?;
        let mut target_name = match self.settings.timezone {
            None => self.get_target_file_stem(&naive_datetime, Local),
            Some(timezone) => self.get_target_file_stem(&naive_datetime, timezone),
        }?;
        target_name.push_str(".");
        target_name.push_str(target_extension);
        Ok(target_name)
    }

    fn get_target_path(&self, source_path: &Path) -> Result<PathBuf, RenameSkip> {
        if source_path.is_dir() {
            return Err(RenameSkip::Directory);
        }
        let target_path = {
            let target_name = self.get_target_name(&source_path)?;
            let parent = source_path.parent().unwrap();
            parent.join(target_name)
        };
        if source_path == target_path {
            Err(RenameSkip::WellNamed)
        } else {
            Ok(target_path)
        }
    }

    fn log_rename_skip(&self, rename_error: &RenameSkip, source_path: &Path) {
        let (log_level, reason) = match rename_error {
            RenameSkip::Extension => (log::Level::Info, "invalid extension"),
            RenameSkip::Directory => (log::Level::Info, "is a directory"),
            RenameSkip::CantGetDate => (log::Level::Warn, "can't determine date"),
            RenameSkip::WellNamed => (log::Level::Debug, "already well named"),
        };
        log!(
            log_level,
            "Skipping file {}: {}",
            source_path.display(),
            reason,
        );
    }

    pub fn get_rename(&self, source_path: PathBuf) -> Option<RenameItem> {
        let target_path = match self.get_target_path(&source_path) {
            Ok(target_path) => target_path,
            Err(rename_error) => {
                self.log_rename_skip(&rename_error, &source_path);
                return None;
            }
        };
        let rename = RenameItem::new(source_path, target_path);
        Some(rename)
    }

    pub fn get_renames(&self, source_dir: &Path) -> Option<Vec<RenameItem>> {
        if source_dir.is_file() {
            let source_path = source_dir.to_path_buf();
            let rename = self.get_rename(source_path);
            return Some(rename.into_iter().collect());
        }
        let direntries = match fs::read_dir(source_dir) {
            Ok(direntries) => direntries,
            Err(err) => {
                warn!("Can't read directory {}: {}", source_dir.display(), err);
                return None;
            }
        };
        let mut direntries: Vec<_> = direntries
            .filter_map(|direntry| match direntry {
                Ok(direntry) => Some(direntry),
                Err(err) => {
                    warn!("Skipping file: {}", err);
                    return None;
                }
            })
            .collect();
        direntries.sort_by_key(fs::DirEntry::path);
        let renames = direntries
            .par_iter()
            .filter_map(|direntry| {
                let source_path = direntry.path();
                self.get_rename(source_path)
            })
            .collect();
        Some(renames)
    }

    pub fn apply_rename(&self, rename: &RenameItem) -> bool {
        match rename.apply() {
            Ok(_) => true,
            Err(err) => {
                warn!(
                    "Can't rename file {} to {}: {}",
                    rename.source_path.display(),
                    rename.target_path.display(),
                    err
                );
                false
            }
        }
    }

    pub fn apply_renames(&self, renames: &[RenameItem]) -> bool {
        let mut result = true;
        for rename in renames {
            result &= self.apply_rename(rename);
        }
        result
    }

    pub fn confirm(&self) -> bool {
        if self.settings.assume_yes {
            return true;
        };
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut input = String::new();
        loop {
            print!("OK? [yN] ");
            stdout.flush().unwrap();
            stdin.read_line(&mut input).unwrap();
            {
                let input = input.trim_right();
                if input == "y" || input == "Y" {
                    return true;
                } else if input == "n" || input == "N" || input == "" {
                    return false;
                } else {
                    println!("Invalid input: {:?}", input);
                }
            }
            input.clear();
        }
    }

    pub fn run(&self, source_dir: &Path) -> i32 {
        let renames = match self.get_renames(source_dir) {
            Some(renames) => renames,
            None => {
                return 1;
            }
        };
        if renames.is_empty() {
            println!("Nothing to do");
            return 0;
        };
        for rename in &renames {
            println!("{}", rename);
        }
        if !self.settings.dry_run && self.confirm() && !self.apply_renames(&renames) {
            return 2;
        };
        0
    }
}
