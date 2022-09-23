use core::cell::RefCell;

use critical_section::Mutex;
use hal::gpio::{OpenDrain, AF4, PB8, PB9};
use hal::i2c::{Error, I2c};
use hal::pac::I2C1;
use hal::prelude::*;

use hal::rcc::Clocks;
use static_cell::{self, StaticCell};

pub type I2c1Handle = I2c<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>;
pub type I2c1HandleProtected = Mutex<RefCell<I2c1Handle>>;
static I2C1_HANDLE: StaticCell<I2c1HandleProtected> = StaticCell::new();

pub fn init_i2c1(
    i2c1: I2C1,
    pins: (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>),
    clocks: &Clocks,
) -> &'static mut I2c1HandleProtected {
    let i2c = I2c::new(i2c1, pins, 400.kHz(), &clocks);

    I2C1_HANDLE.init(Mutex::new(RefCell::new(i2c)))
}

pub trait BlockingI2C {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Error>;
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Error>;
    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Error>;
}

impl BlockingI2C for I2c1Handle {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Error> {
        I2c::write(self, addr, bytes)
    }

    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Error> {
        I2c::read(self, addr, buffer)
    }

    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Error> {
        I2c::write_read(self, addr, bytes, buffer)
    }
}
