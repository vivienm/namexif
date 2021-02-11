use std::error;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::Path;
use std::result;

use chrono::offset::LocalResult;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone};
use derive_more::{Display, From};

#[derive(Debug)]
pub enum TagError {
    Missing,
    Invalid,
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} EXIF tag",
            match self {
                TagError::Missing => "Missing",
                TagError::Invalid => "Invalid",
            },
        )
    }
}

impl error::Error for TagError {}

#[derive(Debug)]
pub enum DateError {
    InvalidLocalDatetime,
    AmbiguousLocalDatetime,
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DateError::InvalidLocalDatetime => "Invalid local date",
                DateError::AmbiguousLocalDatetime => "Ambiguous local date",
            },
        )
    }
}

impl error::Error for DateError {}

#[derive(Debug, Display, From)]
pub enum Error {
    Io(io::Error),
    Exif(exif::Error),
    Tag(TagError),
    Date(DateError),
}

impl error::Error for Error {}

pub type Result<T> = result::Result<T, Error>;

pub struct Image {
    exif: exif::Exif,
}

impl Image {
    fn new(exif: exif::Exif) -> Image {
        Self { exif }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let img_file = File::open(path)?;
        let mut img_buff = io::BufReader::new(img_file);
        let exif = exif::Reader::new().read_from_container(&mut img_buff)?;
        Ok(Self::new(exif))
    }

    fn get_exif_field(&self, tag: exif::Tag) -> Result<&exif::Field> {
        self.exif
            .get_field(tag, exif::In::PRIMARY)
            .ok_or(Error::Tag(TagError::Missing))
    }

    fn get_exif_datetime_with(&self, tag: exif::Tag) -> Result<exif::DateTime> {
        let field = self.get_exif_field(tag)?;
        match field.value {
            exif::Value::Ascii(ref ascii) if !ascii.is_empty() => {
                exif::DateTime::from_ascii(&ascii[0]).map_err(Error::Exif)
            }
            _ => Err(Error::Tag(TagError::Invalid)),
        }
    }

    fn get_naive_datetime_with(&self, tag: exif::Tag) -> Result<NaiveDateTime> {
        let edt = self.get_exif_datetime_with(tag)?;
        let date = NaiveDate::from_ymd(edt.year.into(), edt.month.into(), edt.day.into());
        Ok(date.and_hms(edt.hour.into(), edt.minute.into(), edt.second.into()))
    }

    pub fn get_naive_datetime(&self) -> Result<NaiveDateTime> {
        self.get_naive_datetime_with(exif::Tag::DateTimeOriginal)
    }

    pub fn get_datetime<T>(&self, timezone: &T) -> Result<DateTime<T>>
    where
        T: TimeZone,
    {
        let naive_datetime = self.get_naive_datetime()?;
        match timezone.from_local_datetime(&naive_datetime) {
            LocalResult::None => Err(Error::Date(DateError::InvalidLocalDatetime)),
            LocalResult::Single(datetime) => Ok(datetime),
            LocalResult::Ambiguous(..) => Err(Error::Date(DateError::AmbiguousLocalDatetime)),
        }
    }
}
