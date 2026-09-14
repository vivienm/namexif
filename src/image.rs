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

    fn get_ascii_field(&self, tag: exif::Tag) -> Result<Option<&[u8]>> {
        let Some(field) = self.exif.get_field(tag, exif::In::PRIMARY) else {
            return Ok(None);
        };
        match &field.value {
            exif::Value::Ascii(ascii) if ascii.len() == 1 => Ok(Some(&ascii[0])),
            _ => Err(Error::Tag(TagError::Invalid)),
        }
    }

    fn get_exif_datetime(&self) -> Result<exif::DateTime> {
        let data = self
            .get_ascii_field(exif::Tag::DateTimeOriginal)?
            .ok_or(Error::Tag(TagError::Missing))?;
        let mut datetime = exif::DateTime::from_ascii(data)?;
        if let Some(subsec) = self.get_ascii_field(exif::Tag::SubSecTimeOriginal)? {
            datetime.parse_subsec(subsec)?;
        }
        if let Some(offset) = self.get_ascii_field(exif::Tag::OffsetTimeOriginal)? {
            // EXIF permits blank offsets to represent an unknown time zone.
            match datetime.parse_offset(offset) {
                Err(exif::Error::BlankValue(_)) => {}
                Err(err) => return Err(err.into()),
                Ok(()) => {
                    // The EXIF parser checks syntax but not component ranges.
                    if offset.len() != 6 || &offset[1..3] > b"23" || &offset[4..6] > b"59" {
                        return Err(Error::Tag(TagError::Invalid));
                    }
                }
            }
        }
        Ok(datetime)
    }

    pub fn get_zoned(&self, timezone: &tz::TimeZone) -> Result<jiff::Zoned> {
        let edt = self.get_exif_datetime()?;
        let to_i8 = |v: u8| i8::try_from(v).map_err(|_| Error::OutOfRange);
        let datetime = civil::DateTime::new(
            i16::try_from(edt.year).map_err(|_| Error::OutOfRange)?,
            to_i8(edt.month)?,
            to_i8(edt.day)?,
            to_i8(edt.hour)?,
            to_i8(edt.minute)?,
            to_i8(edt.second)?,
            i32::try_from(edt.nanosecond.unwrap_or(0)).map_err(|_| Error::OutOfRange)?,
        )
        .map_err(|_| Error::OutOfRange)?;

        if let Some(minutes) = edt.offset {
            let offset =
                tz::Offset::from_seconds(i32::from(minutes) * 60).map_err(|_| Error::OutOfRange)?;
            let timestamp = offset
                .to_timestamp(datetime)
                .map_err(|_| Error::OutOfRange)?;
            return Ok(timestamp.to_zoned(timezone.clone()));
        }
        datetime
            .to_zoned(timezone.clone())
            .map_err(|_| Error::InvalidLocalDatetime)
    }
}
