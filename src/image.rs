use std::{fs::File, io, path::Path, result};

use derive_more::{Display, Error, From};
use jiff::{civil, tz};

#[derive(Debug, Display, Error)]
pub enum TagError {
    #[display("Missing EXIF tag")]
    Missing,
    #[display("Invalid EXIF tag")]
    Invalid,
}

#[derive(Debug, Error, Display, From)]
pub enum Error {
    Io(io::Error),
    Exif(exif::Error),
    Tag(TagError),
    #[display("Invalid local date in time zone")]
    InvalidLocalDatetime,
    #[display("Date or time out of range")]
    OutOfRange,
}

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

    fn get_civil_datetime_with(&self, tag: exif::Tag) -> Result<civil::DateTime> {
        let edt = self.get_exif_datetime_with(tag)?;
        let to_i8 = |v: u8| i8::try_from(v).map_err(|_| Error::OutOfRange);
        civil::DateTime::new(
            i16::try_from(edt.year).map_err(|_| Error::OutOfRange)?,
            to_i8(edt.month)?,
            to_i8(edt.day)?,
            to_i8(edt.hour)?,
            to_i8(edt.minute)?,
            to_i8(edt.second)?,
            0,
        )
        .map_err(|_| Error::OutOfRange)
    }

    pub fn get_civil_datetime(&self) -> Result<civil::DateTime> {
        self.get_civil_datetime_with(exif::Tag::DateTimeOriginal)
    }

    pub fn get_zoned(&self, timezone: &tz::TimeZone) -> Result<jiff::Zoned> {
        let datetime = self.get_civil_datetime()?;
        datetime
            .to_zoned(timezone.clone())
            .map_err(|_| Error::InvalidLocalDatetime)
    }
}
