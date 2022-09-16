use hal::i2c::{Error, I2c, Instance, Pins};

pub trait BlockingI2C {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Error>;
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Error>;
    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Error>;
}

impl<I2C, SCL, SDA> BlockingI2C for I2c<I2C, (SCL, SDA)>
where
    I2C: Instance,
    (SCL, SDA): Pins<I2C>,
{
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
