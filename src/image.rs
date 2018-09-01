use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use exif;

#[derive(Debug)]
pub enum ImageError {
    IOError(io::Error),
    ExifError(exif::Error),
    ExifMissingField(exif::Tag),
    ExifInvalidField(exif::Tag),
}

pub struct ImageFile {
    reader: exif::Reader,
}

impl ImageFile {
    fn new(reader: exif::Reader) -> ImageFile {
        ImageFile { reader }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, ImageError> {
        let img_file = File::open(path).map_err(ImageError::IOError)?;
        let mut img_buff = BufReader::new(img_file);
        let reader = exif::Reader::new(&mut img_buff).map_err(ImageError::ExifError)?;
        Ok(Self::new(reader))
    }

    fn get_raw_field(&self, tag: exif::Tag) -> Result<&exif::Field, ImageError> {
        self.reader
            .get_field(tag, false)
            .ok_or_else(|| ImageError::ExifMissingField(tag))
    }

    fn _get_datetime_field(&self, tag: exif::Tag) -> Result<exif::DateTime, ImageError> {
        let field = self.get_raw_field(tag)?;
        match field.value {
            exif::Value::Ascii(ref ascii) if !ascii.is_empty() => {
                exif::DateTime::from_ascii(ascii[0]).map_err(ImageError::ExifError)
            }
            _ => Err(ImageError::ExifInvalidField(exif::Tag::DateTimeOriginal)),
        }
    }

    fn get_datetime_field(&self, tag: exif::Tag) -> Result<NaiveDateTime, ImageError> {
        let edt = self._get_datetime_field(tag)?;
        let date = NaiveDate::from_ymd(edt.year.into(), edt.month.into(), edt.day.into());
        Ok(date.and_hms(edt.hour.into(), edt.minute.into(), edt.second.into()))
    }

    pub fn get_datetime(&self) -> Result<NaiveDateTime, ImageError> {
        self.get_datetime_field(exif::Tag::DateTimeOriginal)
    }
}
