use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use log;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use simplelog;

use imgfile::ImageFile;
use rename::Rename;

const JPEG_EXTENSION: &'static str = "jpg";
const JPEG_EXTENSIONS: [&'static str; 4] = [JPEG_EXTENSION, "JPG", "jpeg", "JPEG"];
const TIFF_EXTENSION: &'static str = "tiff";
const TIFF_EXTENSIONS: [&'static str; 4] = [TIFF_EXTENSION, "tif", "TIF", "TIFF"];

pub struct AppBuilder<'a> {
    name_format: &'a str,
    timezone: Option<Tz>,
    log_level: log::LevelFilter,
    dry_run: bool,
    assume_yes: bool,
}

impl<'a> AppBuilder<'a> {
    pub fn new() -> Self {
        Self {
            name_format: "%Y-%m-%dT%H:%M:%S%z",
            timezone: None,
            log_level: log::LevelFilter::Info,
            dry_run: false,
            assume_yes: false,
        }
    }

    pub fn with_name_format(&mut self, name_format: &'a str) -> &mut Self {
        self.name_format = name_format;
        self
    }

    pub fn with_log_level(&mut self, log_level: log::LevelFilter) -> &mut Self {
        self.log_level = log_level;
        self
    }

    pub fn with_timezone(&mut self, timezone: Tz) -> &mut Self {
        self.timezone = Some(timezone);
        self
    }

    pub fn with_dry_run(&mut self, dry_run: bool) -> &mut Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_assume_yes(&mut self, assume_yes: bool) -> &mut Self {
        self.assume_yes = assume_yes;
        self
    }

    pub fn build(&self) -> App<'a> {
        App {
            name_format: self.name_format,
            timezone: self.timezone,
            log_level: self.log_level,
            dry_run: self.dry_run,
            assume_yes: self.assume_yes,
        }
    }
}

pub struct App<'a> {
    name_format: &'a str,
    timezone: Option<Tz>,
    log_level: log::LevelFilter,
    dry_run: bool,
    assume_yes: bool,
}

enum RenameError {
    UnsupportedFileFormat,
    IsADirectory,
    CantExtractDate,
    DontNeedRenaming,
}

impl<'a> App<'a> {
    pub fn init(&self) {
        simplelog::TermLogger::init(self.log_level, simplelog::Config::default()).unwrap();
    }

    fn get_target_extension(&self, source_path: &Path) -> Result<&str, RenameError> {
        let source_extension = source_path
            .extension()
            .and_then(OsStr::to_str)
            .ok_or(RenameError::UnsupportedFileFormat)?;
        if JPEG_EXTENSIONS.contains(&source_extension) {
            Ok(JPEG_EXTENSION)
        } else if TIFF_EXTENSIONS.contains(&source_extension) {
            Ok(TIFF_EXTENSION)
        } else {
            Err(RenameError::UnsupportedFileFormat)
        }
    }

    fn get_target_file_stem<T>(
        &self,
        naive_datetime: &NaiveDateTime,
        timezone: T,
    ) -> Result<String, RenameError>
    where
        T: TimeZone,
        T::Offset: fmt::Display,
    {
        let datetime = timezone
            .from_local_datetime(naive_datetime)
            .single()
            .ok_or(RenameError::CantExtractDate)?;
        Ok(datetime.format(self.name_format).to_string())
    }

    fn get_target_name(&self, source_path: &Path) -> Result<String, RenameError> {
        let target_extension = self.get_target_extension(&source_path)?;
        let naive_datetime = ImageFile::open(source_path)
            .and_then(|imgfile| imgfile.get_datetime())
            .map_err(|_| RenameError::CantExtractDate)?;
        let mut target_name = match self.timezone {
            None => self.get_target_file_stem(&naive_datetime, Local),
            Some(timezone) => self.get_target_file_stem(&naive_datetime, timezone),
        }?;
        target_name.push_str(".");
        target_name.push_str(target_extension);
        Ok(target_name)
    }

    fn get_target_path(&self, source_path: &Path) -> Result<PathBuf, RenameError> {
        if source_path.is_dir() {
            return Err(RenameError::IsADirectory);
        }
        let target_path = {
            let target_name = self.get_target_name(&source_path)?;
            let parent = source_path.parent().unwrap();
            parent.join(target_name)
        };
        if source_path == target_path {
            Err(RenameError::DontNeedRenaming)
        } else {
            Ok(target_path)
        }
    }

    fn log_rename_erroror(&self, rename_error: &RenameError, source_path: &Path) {
        let (log_level, reason) = match rename_error {
            RenameError::UnsupportedFileFormat => (log::Level::Info, "invalid extension"),
            RenameError::IsADirectory => (log::Level::Info, "is a directory"),
            RenameError::CantExtractDate => (log::Level::Warn, "can't determine date"),
            RenameError::DontNeedRenaming => (log::Level::Debug, "already well named"),
        };
        log!(
            log_level,
            "Skipping file {}: {}",
            source_path.display(),
            reason,
        );
    }

    pub fn get_rename(&self, source_path: PathBuf) -> Option<Rename> {
        let target_path = match self.get_target_path(&source_path) {
            Ok(target_path) => target_path,
            Err(rename_error) => {
                self.log_rename_erroror(&rename_error, &source_path);
                return None;
            }
        };
        let rename = Rename::new(source_path, target_path);
        Some(rename)
    }

    pub fn get_renames(&self, source_dir: &Path) -> Option<Vec<Rename>> {
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

    pub fn apply_rename(&self, rename: &Rename) -> bool {
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

    pub fn apply_renames(&self, renames: &[Rename]) -> bool {
        let mut result = true;
        for rename in renames {
            result &= self.apply_rename(rename);
        }
        result
    }

    pub fn confirm(&self) -> bool {
        if self.assume_yes {
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
        if !self.dry_run && self.confirm() && !self.apply_renames(&renames) {
            return 2;
        };
        0
    }
}
