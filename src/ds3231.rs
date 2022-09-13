use chrono::prelude::*;

use crate::i2c::BlockingI2C;

const I2C_ADDRESS: u8 = 0b01101000;

#[repr(u8)]
pub enum Register {
    Seconds = 0x00,
    Minutes = 0x01,
    Hours = 0x02,
}

#[repr(u8)]
enum HoursMasks {
    /// 12(True) or 24(False) hours format
    H12_24 = 0b01000000,
    /// PM(True) AM (False)
    AmPm = 0b00100000,
}

#[derive(Debug)]
pub enum Error {
    I2CError,
}

pub struct DS3231 {
    time: DateTime<Utc>,
}

impl DS3231 {
    pub fn new() -> Self {
        Self {
            time: DateTime::default(),
        }
    }

    pub fn update_time<B: BlockingI2C>(&mut self, bus: &mut B) -> Result<(), Error> {
        let data = self.read_registers(bus)?;

        let secs = Self::bcd_to_decimal(data[Register::Seconds as usize]);
        self.time = self.time.with_second(secs as u32).unwrap();

        let mins = Self::bcd_to_decimal(data[Register::Minutes as usize]);
        self.time = self.time.with_minute(mins as u32).unwrap();

        let hours = Self::hours_to_decimal(data[Register::Hours as usize]);
        self.time = self.time.with_hour(hours as u32).unwrap();

        Ok(())
    }

    pub fn time(&self) -> &DateTime<Utc> {
        &self.time
    }

    fn read_registers<B: BlockingI2C>(&mut self, bus: &mut B) -> Result<[u8; 7], Error> {
        let mut buf = [0_u8; 7];
        if let Err(_) = bus.write_read(I2C_ADDRESS, &[0], &mut buf) {
            return Err(Error::I2CError);
        }

        Ok(buf)
    }

    fn bcd_to_decimal(bcd: u8) -> u8 {
        ((bcd & 0b11110000) >> 4) * 10 + (bcd & 0b00001111)
    }

    fn hours_to_decimal(bcd: u8) -> u8 {
        let is_ampm_format = (HoursMasks::H12_24 as u8) & bcd;

        if is_ampm_format != 0 {
            if (HoursMasks::AmPm as u8) & bcd != 0 {
                // If is PM
                return 12
                    + Self::bcd_to_decimal(
                        bcd & !((HoursMasks::AmPm as u8) | (HoursMasks::H12_24 as u8)),
                    );
            } else {
                return Self::bcd_to_decimal(
                    bcd & !((HoursMasks::AmPm as u8) | (HoursMasks::H12_24 as u8)),
                );
            }
        }

        return Self::bcd_to_decimal(bcd & !(HoursMasks::H12_24 as u8));
    }
}
