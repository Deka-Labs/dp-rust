use core::cell::RefCell;

use cortex_m::asm::nop;
use critical_section::Mutex;
use stm32f4xx_hal::gpio::{Output, Pin, PushPull};

use crate::i2c::BlockingI2C;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, primitives::Rectangle};

/// We use only this address. Additional 0x3D unsupported
const I2C_ADDRESS: u8 = 0x3C;
const SCREEN_WIDTH: usize = 128;
const SCREEN_HEIGHT: usize = 64;
const PAGE_COUNT: usize = 64 / 8;
/// Buffer size - 128x64 resolutions /8 - each pixel is one bit, not byte.
const BUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT / 8;

#[derive(Debug)]
pub enum OperationError {
    I2CError,
}

pub struct SSD1306<'bus, PIN, I2C: BlockingI2C + 'bus> {
    reset_pin: PIN,
    i2c: &'bus Mutex<RefCell<I2C>>,

    buffer: [u8; BUFFER_SIZE + 1], // The first byte is Control byte 0x40
}

impl<'bus, const P: char, const N: u8, I2C: BlockingI2C>
    SSD1306<'bus, Pin<P, N, Output<PushPull>>, I2C>
{
    /// Creates SSD1306 driver
    pub fn new(reset_pin: Pin<P, N, Output<PushPull>>, i2c: &'bus Mutex<RefCell<I2C>>) -> Self {
        Self {
            reset_pin: reset_pin,
            i2c,
            buffer: [0x40; BUFFER_SIZE + 1],
        }
    }

    /// Initializes SSD1306
    pub fn init(&mut self) -> Result<(), OperationError> {
        self.reset_pin.set_high(); // Reset pin must be on

        // Standart start up commands
        self.send_command(0xAE)?; /*display off*/
        self.send_command(0x20)?;
        self.send_command(0x00)?;

        self.send_command(0xC8)?; /*Com scan direction*/

        self.send_command(0x00)?; /*set lower column address*/
        self.send_command(0x10)?; /*set higher column address*/
        self.send_command(0x40)?; /*set display start line*/

        self.send_command(0xB0)?; /*set page address*/
        self.send_command(0x81)?; /*contract control*/
        self.send_command(0xcf)?; /*128*/

        self.send_command(0xA1)?; /*set segment remap*/

        self.send_command(0xA6)?; /*normal / reverse*/

        self.send_command(0xA8)?; /*multiplex ratio*/
        self.send_command(0x3F)?; /*duty = 1/64*/

        self.send_command(0xA4)?; /* Display RAM content*/

        self.send_command(0xD3)?; /*set display offset*/
        self.send_command(0x00)?;
        self.send_command(0xD5)?; /*set osc division*/
        self.send_command(0x80)?;

        self.send_command(0xD9)?; /*set pre-charge period*/
        self.send_command(0x22)?;

        self.send_command(0xDA)?; /*set COM pins*/
        self.send_command(0x12)?;

        self.send_command(0xdb)?; /*set vcomh*/
        self.send_command(0x20)?;
        self.send_command(0x8D)?; /*set charge pump disable*/
        self.send_command(0x14)?;

        self.send_command(0xAF)?; /*display ON*/

        self.clear();
        self.send_image()?;

        return Ok(());
    }

    pub fn clear(&mut self) {
        self.buffer[1..].fill(0) // Skip 1 data byte
    }

    pub fn dot(&mut self, p: Point, filled: bool) {
        if p.x < 0 || (SCREEN_WIDTH as i32) <= p.x {
            return;
        }
        if p.y < 0 || (SCREEN_HEIGHT as i32) <= p.y {
            return;
        }

        let page = p.y / (PAGE_COUNT as i32);
        let index = page * (SCREEN_WIDTH as i32) + p.x + 1; // +1 skip 1 data byte

        if filled {
            self.buffer[index as usize] |= 1 << (p.y % 8);
        } else {
            self.buffer[index as usize] &= !(1 << (p.y % 8));
        }
    }

    pub fn swap(&mut self) {
        while self.send_image().is_err() {
            self.reset_position()
        }
    }

    fn reset_position(&mut self) {
        while self
            .send_command(0x21)
            .and(self.send_command(0))
            .and(self.send_command(127))
            .and(self.send_command(0x22))
            .and(self.send_command(0))
            .and(self.send_command(7))
            .is_err()
        {
            nop();
        }
    }

    fn send_command(&mut self, cmd: u8) -> Result<(), OperationError> {
        critical_section::with(|cs| {
            let mut bus = self.i2c.borrow(cs).borrow_mut();

            if let Err(_) = bus.write(I2C_ADDRESS, &[0x0, cmd]) {
                return Err(OperationError::I2CError);
            }

            Ok(())
        })
    }

    fn send_image(&mut self) -> Result<(), OperationError> {
        critical_section::with(|cs| {
            let mut bus = self.i2c.borrow(cs).borrow_mut();

            if let Err(_) = bus.write(I2C_ADDRESS, &self.buffer) {
                return Err(OperationError::I2CError);
            }

            Ok(())
        })
    }
}

impl<'bus, const P: char, const N: u8, I2C: BlockingI2C> Dimensions
    for SSD1306<'bus, Pin<P, N, Output<PushPull>>, I2C>
{
    fn bounding_box(&self) -> Rectangle {
        Rectangle {
            top_left: Point { x: 0, y: 0 },
            size: Size {
                width: SCREEN_WIDTH as u32,
                height: SCREEN_HEIGHT as u32,
            },
        }
    }
}

impl<'bus, const P: char, const N: u8, I2C: BlockingI2C> DrawTarget
    for SSD1306<'bus, Pin<P, N, Output<PushPull>>, I2C>
{
    type Color = BinaryColor;
    type Error = OperationError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for p in pixels {
            self.dot(p.0, p.1.is_on())
        }

        Ok(())
    }
}
