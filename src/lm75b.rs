use crate::i2c::BlockingI2C;

#[derive(Debug)]
pub enum Error {
    BusError,
}

pub struct LM75B {
    address: u8,
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

        Self { address }
    }

    pub fn temperature<B: BlockingI2C>(&self, bus: &mut B) -> Result<f32, Error> {
        let mut buf = [0_u8; 2];
        if let Err(_) = bus.write_read(self.address, &[0], &mut buf) {
            return Err(Error::BusError);
        }

        let raw_temp = i16::from_be_bytes(buf) / 32; // 2^5, 5 to remove last 5 zeros in binary repr.
        Ok(raw_temp as f32 * 0.125)
    }
}
