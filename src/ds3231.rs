use core::cell::RefCell;

use chrono::prelude::*;
use critical_section::Mutex;

use crate::i2c_async::NonBlockingI2C;

const I2C_ADDRESS: u8 = 0b01101000;
const REGISTER_COUNT: usize = 7;

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

#[derive(Debug)]
pub struct DS3231<I2C: NonBlockingI2C + 'static> {
    i2c: &'static I2C,
}

impl<I2C: NonBlockingI2C> DS3231<I2C> {
    pub fn new(i2c: &'static I2C) -> Self {
        Self { i2c }
    }

    pub fn update_time(&self) -> Result<DateTime<Utc>, Error> {
        let data = self.read_registers()?;

        let mut time: DateTime<Utc> = Default::default();

        let secs = bcd_to_decimal(data[Register::Seconds as usize]);
        time = time.with_second(secs as u32).unwrap();

        let mins = bcd_to_decimal(data[Register::Minutes as usize]);
        time = time.with_minute(mins as u32).unwrap();

        let hours = hours_to_decimal(data[Register::Hours as usize]);
        time = time.with_hour(hours as u32).unwrap();

        Ok(time)
    }

    pub fn set_time(&self, time: DateTime<Utc>) -> Result<(), Error> {
        let mut data = [0_u8; REGISTER_COUNT];
        data[Register::Seconds as usize] = decimal_to_bcd(time.second() as u8);
        data[Register::Minutes as usize] = decimal_to_bcd(time.minute() as u8);
        // Store in 24H format
        data[Register::Hours as usize] = decimal_to_bcd(time.hour() as u8);

        self.write_registers(data)?;

        Ok(())
    }

    fn read_registers(&self) -> Result<[u8; REGISTER_COUNT], Error> {
        let mut buf = [0_u8; REGISTER_COUNT];

        let mut future_res = self.i2c.write_read_async(I2C_ADDRESS, &[0], &mut buf);
        while let Err(_) = future_res {
            future_res = self.i2c.write_read_async(I2C_ADDRESS, &[0], &mut buf);
        }

        future_res.unwrap().block().ok();

        Ok(buf)
    }

    fn write_registers(&self, regs: [u8; REGISTER_COUNT]) -> Result<(), Error> {
        let mut buf = [0_u8; REGISTER_COUNT + 1];
        buf[1..].copy_from_slice(&regs);

        let mut future_res = self.i2c.write_async(I2C_ADDRESS, &buf);
        while let Err(_) = future_res {
            future_res = self.i2c.write_async(I2C_ADDRESS, &buf);
        }

        future_res.unwrap().block().ok();

        Ok(())
    }
}

impl<I2C: NonBlockingI2C> Clone for DS3231<I2C> {
    fn clone(&self) -> Self {
        Self { i2c: self.i2c }
    }
}

fn bcd_to_decimal(bcd: u8) -> u8 {
    ((bcd & 0b11110000) >> 4) * 10 + (bcd & 0b00001111)
}

fn decimal_to_bcd(d: u8) -> u8 {
    (d / 10 << 4) | d % 10
}

fn hours_to_decimal(bcd: u8) -> u8 {
    let is_ampm_format = (HoursMasks::H12_24 as u8) & bcd;

    if is_ampm_format != 0 {
        if (HoursMasks::AmPm as u8) & bcd != 0 {
            // If is PM
            return 12
                + bcd_to_decimal(bcd & !((HoursMasks::AmPm as u8) | (HoursMasks::H12_24 as u8)));
        } else {
            return bcd_to_decimal(bcd & !((HoursMasks::AmPm as u8) | (HoursMasks::H12_24 as u8)));
        }
    }

    return bcd_to_decimal(bcd & !(HoursMasks::H12_24 as u8));
}
