use core::cell::RefCell;

use chrono::prelude::*;

use critical_section::Mutex;

use crate::i2c::BlockingI2C;

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
    Busy,
}

#[derive(Debug)]
pub struct DS3231<I2C: BlockingI2C + 'static> {
    i2c: &'static Mutex<RefCell<I2C>>,
}

impl<I2C: BlockingI2C> DS3231<I2C> {
    pub fn new(i2c: &'static Mutex<RefCell<I2C>>) -> Self {
        Self { i2c }
    }

    pub fn update_time(&self) -> Result<DateTime<Utc>, Error> {
        let mut res = self.read_registers();
        while let Err(Error::Busy) = res {
            res = self.read_registers();
        }

        let data = res.unwrap();

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

        let mut res = self.write_registers(&data);
        while let Err(Error::Busy) = res {
            res = self.write_registers(&data);
        }

        Ok(())
    }

    fn read_registers(&self) -> Result<[u8; REGISTER_COUNT], Error> {
        let mut buf = [0_u8; REGISTER_COUNT];

        critical_section::with(|cs| {
            let mut bus = self.i2c.borrow(cs).borrow_mut();

            if let Err(e) = bus.write_read(I2C_ADDRESS, &[0], &mut buf) {
                if e == hal::i2c::Error::Busy {
                    return Err(Error::Busy);
                }
                return Err(Error::I2CError);
            }

            Ok(buf)
        })
    }

    fn write_registers(&self, regs: &[u8; REGISTER_COUNT]) -> Result<(), Error> {
        let mut buf = [0_u8; REGISTER_COUNT + 1];
        buf[1..].copy_from_slice(regs);

        critical_section::with(|cs| {
            let mut bus = self.i2c.borrow(cs).borrow_mut();

            if let Err(e) = bus.write(I2C_ADDRESS, &buf) {
                if e == hal::i2c::Error::Busy {
                    return Err(Error::Busy);
                }
                return Err(Error::I2CError);
            }

            Ok(())
        })
    }
}

impl<I2C: BlockingI2C> Clone for DS3231<I2C> {
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
