use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use exif;

#[derive(Debug)]
pub enum TagErrorKind {
    Missing,
    Invalid,
}

#[derive(Debug)]
pub struct TagError {
    pub kind: TagErrorKind,
    pub tag: exif::Tag,
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} EXIF tag {}",
            match self.kind {
                TagErrorKind::Missing => "Missing",
                TagErrorKind::Invalid => "Invalid",
            },
            self.tag,
        )
    }
}

impl Error for TagError {}

#[derive(Debug)]
pub enum ImageError {
    Io(io::Error),
    Exif(exif::Error),
    Tag(TagError),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageError::Io(err) => err.fmt(f),
            ImageError::Exif(err) => err.fmt(f),
            ImageError::Tag(err) => err.fmt(f),
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

    fn get_exif_field(&self, tag: exif::Tag) -> ImageResult<&exif::Field> {
        self.reader.get_field(tag, false).ok_or_else(|| {
            ImageError::Tag(TagError {
                kind: TagErrorKind::Missing,
                tag,
            })
        })
    }

    fn get_exif_datetime_with(&self, tag: exif::Tag) -> ImageResult<exif::DateTime> {
        let field = self.get_exif_field(tag)?;
        match field.value {
            exif::Value::Ascii(ref ascii) if !ascii.is_empty() => {
                exif::DateTime::from_ascii(ascii[0]).map_err(ImageError::Exif)
            }
            _ => Err(ImageError::Tag(TagError {
                kind: TagErrorKind::Invalid,
                tag,
            })),
        }
    }

    fn get_naive_datetime_with(&self, tag: exif::Tag) -> ImageResult<NaiveDateTime> {
        let edt = self.get_exif_datetime_with(tag)?;
        let date = NaiveDate::from_ymd(edt.year.into(), edt.month.into(), edt.day.into());
        Ok(date.and_hms(edt.hour.into(), edt.minute.into(), edt.second.into()))
    }

    pub fn get_naive_datetime(&self) -> ImageResult<NaiveDateTime> {
        self.get_naive_datetime_with(exif::Tag::DateTimeOriginal)
    }
}
