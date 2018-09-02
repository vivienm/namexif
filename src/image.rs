use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use exif;

#[derive(Debug)]
pub enum ImageError {
    Io(io::Error),
    Exif(exif::Error),
    MissingTag(exif::Tag),
    InvalidTag(exif::Tag),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageError::Io(err) => err.fmt(f),
            ImageError::Exif(err) => err.fmt(f),
            ImageError::MissingTag(tag) => write!(f, "Missing EXIF tag {}", tag),
            ImageError::InvalidTag(tag) => write!(f, "Invalid EXIF tag {}", tag),
        }
    }
}

impl Error for ImageError {}

pub type ImageResult<T> = Result<T, ImageError>;

pub struct ImageFile {
    reader: exif::Reader,
}

impl ImageFile {
    fn new(reader: exif::Reader) -> ImageFile {
        Self { reader }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> ImageResult<Self> {
        let img_file = File::open(path).map_err(ImageError::Io)?;
        let mut img_buff = BufReader::new(img_file);
        let reader = exif::Reader::new(&mut img_buff).map_err(ImageError::Exif)?;
        Ok(Self::new(reader))
    }

    fn get_raw_field(&self, tag: exif::Tag) -> ImageResult<&exif::Field> {
        self.reader
            .get_field(tag, false)
            .ok_or_else(|| ImageError::MissingTag(tag))
    }

    fn _get_datetime_tag(&self, tag: exif::Tag) -> ImageResult<exif::DateTime> {
        let field = self.get_raw_field(tag)?;
        match field.value {
            exif::Value::Ascii(ref ascii) if !ascii.is_empty() => {
                exif::DateTime::from_ascii(ascii[0]).map_err(ImageError::Exif)
            }
            _ => Err(ImageError::InvalidTag(tag)),
        }
    }

    fn get_datetime_tag(&self, tag: exif::Tag) -> ImageResult<NaiveDateTime> {
        let edt = self._get_datetime_tag(tag)?;
        let date = NaiveDate::from_ymd(edt.year.into(), edt.month.into(), edt.day.into());
        Ok(date.and_hms(edt.hour.into(), edt.minute.into(), edt.second.into()))
    }

    pub fn get_datetime(&self) -> ImageResult<NaiveDateTime> {
        self.get_datetime_tag(exif::Tag::DateTimeOriginal)
    }
}
