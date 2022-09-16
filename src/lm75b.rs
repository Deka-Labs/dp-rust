use crate::i2c::BlockingI2C;

const AVG_BUFFER_SIZE: usize = 10;

#[derive(Debug)]
pub enum Error {
    BusError,
}

pub struct LM75B {
    address: u8,

    avg: [f32; AVG_BUFFER_SIZE],
    avg_pointer: usize,
    /// If true - all elements in `avg` filled with real data
    avg_full: bool,
}

impl LM75B {
    /// new crates LM75B driver
    /// # Arguments
    ///
    /// * `address_config` - A state of 3 address pins; [A0, A1, A2]; Refer LM75B docs
    pub fn new(address_config: [bool; 3]) -> Self {
        let address = {
            let mut a = 0b01001000;
            for i in 0..3 {
                if address_config[i] {
                    a |= 1 << i;
                }
            }
            a
        };

        Self {
            address,
            avg: [0.0_f32; AVG_BUFFER_SIZE],
            avg_pointer: 0,
            avg_full: false,
        }
    }

    pub fn temperature<B: BlockingI2C>(&mut self, bus: &mut B) -> Result<f32, Error> {
        let mut buf = [0_u8; 2];
        if let Err(_) = bus.write_read(self.address, &[0], &mut buf) {
            return Err(Error::BusError);
        }

        let raw_temp = i16::from_be_bytes(buf) >> 5;
        let real_temp = raw_temp as f32 * 0.125;

        self.avg[self.avg_pointer] = real_temp;
        self.avg_pointer += 1;
        if self.avg_pointer == AVG_BUFFER_SIZE {
            self.avg_full = true;
        }
        self.avg_pointer %= AVG_BUFFER_SIZE;

        Ok(self.get_filtered_temp())
    }

    fn get_filtered_temp(&self) -> f32 {
        if self.avg_full {
            return self.avg.iter().sum::<f32>() / (self.avg.len() as f32);
        }

        if self.avg_pointer == 0 {
            return 0.0_f32;
        }

        self.avg[..self.avg_pointer].iter().sum::<f32>() / self.avg_pointer as f32
    }
}
