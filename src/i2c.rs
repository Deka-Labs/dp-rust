use hal::gpio::{OpenDrain, AF4, PB8, PB9};
use hal::i2c::{Error, I2CDma};
use hal::pac::I2C1;
use stm32f4xx_hal::dma::Stream1;
use stm32f4xx_hal::pac::DMA1;

pub type I2c1Handle = I2CDma<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>), Stream1<DMA1>, 0>;

pub trait BlockingI2C {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Error>;
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Error>;
    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Error>;
}

impl BlockingI2C for I2c1Handle {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Error> {
        I2c1Handle::write(self, addr, bytes)
    }

    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Error> {
        I2c1Handle::read(self, addr, buffer)
    }

    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Error> {
        I2c1Handle::write_read(self, addr, bytes, buffer)
    }
}
