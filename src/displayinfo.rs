use chrono::{prelude::*, Duration};

pub type DateTime = chrono::DateTime<Utc>;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default)]
pub enum RTCField {
    #[default]
    Hours = 0,
    Minutes,
}

impl RTCField {
    pub fn next(&mut self) {
        use RTCField::*;
        *self = match self {
            Hours => Minutes,
            Minutes => Hours,
        }
    }

    pub fn prev(&mut self) {
        use RTCField::*;
        *self = match self {
            Hours => Minutes,
            Minutes => Hours,
        }
    }
}

#[derive(Default)]
pub struct DisplayInfo {
    datetime: DateTime,

    temperature: f32,

    edit_field: RTCField,
}

impl DisplayInfo {
    /// Creates information for `dt` time
    pub fn from_datetime(dt: &DateTime) -> Self {
        Self {
            datetime: dt.clone(),
            temperature: 0.0,
            edit_field: RTCField::Hours,
        }
    }

    /// Returns current displayed time
    pub fn datetime(&self) -> &DateTime {
        &self.datetime
    }

    /// Reset seconds to zero and returns datetime
    pub fn reset_seconds(&mut self) -> &DateTime {
        self.datetime = self.datetime.with_second(0).unwrap();
        &self.datetime
    }

    /// Edits specified field
    pub fn add_time(&mut self, duration: i64) {
        match self.edit_field {
            RTCField::Hours => self.datetime += Duration::hours(duration),
            RTCField::Minutes => self.datetime += Duration::minutes(duration),
        }
    }

    /// Edits specified field
    pub fn sub_time(&mut self, duration: i64) {
        match self.edit_field {
            RTCField::Hours => self.datetime -= Duration::hours(duration),
            RTCField::Minutes => self.datetime -= Duration::minutes(duration),
        }
    }

    pub fn next_field(&mut self) {
        self.edit_field.next()
    }

    pub fn prev_field(&mut self) {
        self.edit_field.prev()
    }

    pub fn field(&self) -> RTCField {
        self.edit_field
    }

    /// Adds 1 second to internal date
    pub fn tick(&mut self) {
        self.datetime += Duration::seconds(1);
    }

    pub fn set_temperature(&mut self, temp: f32) {
        self.temperature = temp;
    }

    pub fn temperature(&self) -> f32 {
        self.temperature
    }
}
