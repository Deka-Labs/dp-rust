mod futures;
use cortex_m_semihosting::hprintln;
pub use futures::I2COperationFuture;
mod states;
mod transaction;

use core::mem::transmute;

use cortex_m::peripheral::NVIC;
use hal::{
    i2c::{DutyCycle, Instance, Mode, NoAcknowledgeSource, Pins},
    pac::{i2c1, I2C1, RCC},
    rcc::Clocks,
    time::Hertz,
};
use heapless::spsc::Queue;

use self::{
    states::State,
    transaction::{Command, Transaction},
};

pub trait NonBlockingI2C {
    fn write_read_async<'b>(
        &self,
        addr: u8,
        to_send: &'b [u8],
        to_recv: &'b mut [u8],
    ) -> Result<I2COperationFuture, Error>;

    fn write_async<'b>(&self, addr: u8, to_send: &'b [u8]) -> Result<I2COperationFuture, Error>;

    fn read_async<'b>(&self, addr: u8, to_recv: &'b mut [u8]) -> Result<I2COperationFuture, Error>;
}

pub struct I2c<I2C: Instance, PINS> {
    i2c: I2C,
    pins: PINS,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[non_exhaustive]
pub enum Error {
    Overrun,
    NoAcknowledge(NoAcknowledgeSource),
    Timeout,
    // Note: The Bus error type is not currently returned, but is maintained for compatibility.
    Bus,
    Crc,
    ArbitrationLoss,
    Busy,
}

impl Error {
    pub(crate) fn nack_addr(self) -> Self {
        match self {
            Error::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Error::NoAcknowledge(NoAcknowledgeSource::Address)
            }
            e => e,
        }
    }
    pub(crate) fn nack_data(self) -> Self {
        match self {
            Error::NoAcknowledge(NoAcknowledgeSource::Unknown) => {
                Error::NoAcknowledge(NoAcknowledgeSource::Data)
            }
            e => e,
        }
    }
}

#[derive(Debug)]
pub enum I2CEventInterrupt {
    StartBitSent,
    AddressSent,
    DataByteTransferFinished,

    Unknown,
}

impl<I2C, SCL, SDA> I2c<I2C, (SCL, SDA)>
where
    I2C: Instance,
    (SCL, SDA): Pins<I2C>,
{
    pub fn new(i2c: I2C, mut pins: (SCL, SDA), mode: impl Into<Mode>, clocks: &Clocks) -> Self {
        unsafe {
            // NOTE(unsafe) this reference will only be used for atomic writes with no side effects.
            let rcc = &(*RCC::ptr());

            // Enable and reset clock.
            I2C::enable(rcc);
            I2C::reset(rcc);
        }

        pins.set_alt_mode();

        let i2c = I2c { i2c, pins };
        i2c.i2c_init(mode, clocks.pclk1());
        i2c
    }

    pub fn release(mut self) -> (I2C, (SCL, SDA)) {
        self.pins.restore_mode();

        (self.i2c, (self.pins.0, self.pins.1))
    }
}

impl<I2C: Instance, PINS> I2c<I2C, PINS> {
    fn i2c_init(&self, mode: impl Into<Mode>, pclk: Hertz) {
        let mode = mode.into();
        // Make sure the I2C unit is disabled so we can configure it
        self.i2c.cr1.modify(|_, w| w.pe().clear_bit());

        // Calculate settings for I2C speed modes
        let clock = pclk.raw();
        let clc_mhz = clock / 1_000_000;
        assert!((2..=50).contains(&clc_mhz));

        // Configure bus frequency into I2C peripheral
        self.i2c
            .cr2
            .write(|w| unsafe { w.freq().bits(clc_mhz as u8) });

        let trise = match mode {
            Mode::Standard { .. } => clc_mhz + 1,
            Mode::Fast { .. } => clc_mhz * 300 / 1000 + 1,
        };

        // Configure correct rise times
        self.i2c.trise.write(|w| w.trise().bits(trise as u8));

        match mode {
            // I2C clock control calculation
            Mode::Standard { frequency } => {
                let ccr = (clock / (frequency.raw() * 2)).max(4);

                // Set clock to standard mode with appropriate parameters for selected speed
                self.i2c.ccr.write(|w| unsafe {
                    w.f_s()
                        .clear_bit()
                        .duty()
                        .clear_bit()
                        .ccr()
                        .bits(ccr as u16)
                });
            }
            Mode::Fast {
                frequency,
                duty_cycle,
            } => match duty_cycle {
                DutyCycle::Ratio2to1 => {
                    let ccr = (clock / (frequency.raw() * 3)).max(1);

                    // Set clock to fast mode with appropriate parameters for selected speed (2:1 duty cycle)
                    self.i2c.ccr.write(|w| unsafe {
                        w.f_s().set_bit().duty().clear_bit().ccr().bits(ccr as u16)
                    });
                }
                DutyCycle::Ratio16to9 => {
                    let ccr = (clock / (frequency.raw() * 25)).max(1);

                    // Set clock to fast mode with appropriate parameters for selected speed (16:9 duty cycle)
                    self.i2c.ccr.write(|w| unsafe {
                        w.f_s().set_bit().duty().set_bit().ccr().bits(ccr as u16)
                    });
                }
            },
        }

        // Enable the I2C processing
        self.i2c.cr1.modify(|_, w| w.pe().set_bit());
    }

    #[inline(always)]
    fn check_and_clear_error_flags(reg: &i2c1::RegisterBlock) -> Result<i2c1::sr1::R, Error> {
        // Note that flags should only be cleared once they have been registered. If flags are
        // cleared otherwise, there may be an inherent race condition and flags may be missed.
        let sr1 = reg.sr1.read();

        if sr1.timeout().bit_is_set() {
            reg.sr1.modify(|_, w| w.timeout().clear_bit());
            return Err(Error::Timeout);
        }

        if sr1.pecerr().bit_is_set() {
            reg.sr1.modify(|_, w| w.pecerr().clear_bit());
            return Err(Error::Crc);
        }

        if sr1.ovr().bit_is_set() {
            reg.sr1.modify(|_, w| w.ovr().clear_bit());
            return Err(Error::Overrun);
        }

        if sr1.af().bit_is_set() {
            reg.sr1.modify(|_, w| w.af().clear_bit());
            return Err(Error::NoAcknowledge(NoAcknowledgeSource::Unknown));
        }

        if sr1.arlo().bit_is_set() {
            reg.sr1.modify(|_, w| w.arlo().clear_bit());
            return Err(Error::ArbitrationLoss);
        }

        // The errata indicates that BERR may be incorrectly detected. It recommends ignoring and
        // clearing the BERR bit instead.
        if sr1.berr().bit_is_set() {
            reg.sr1.modify(|_, w| w.berr().clear_bit());
        }

        Ok(sr1)
    }

    #[inline(always)]
    pub unsafe fn handle_event_interrupt() {
        let registers = { &*I2C1::ptr() };
        Self::handle_event_interrupt_impl(&registers);
    }

    #[inline(always)]
    pub unsafe fn handle_error_interrupt() {
        let registers = { &*I2C1::ptr() };
        Self::handle_error_interrupt_impl(&registers);
    }

    #[inline(always)]
    fn handle_event_interrupt_impl(reg: &i2c1::RegisterBlock) {
        {
            NVIC::unpend(hal::interrupt::I2C1_EV)
        }

        let ctx = unsafe { &mut TRANSACTION };

        // Determinate reason of interrupt
        let reason = Self::event_interupt_reason(reg);
        match reason {
            I2CEventInterrupt::StartBitSent => {
                *ctx.state_mut() = State::StartGenerated;
                if ctx.is_read() {
                    Self::send_address(reg, ctx.address(), 1);
                } else {
                    Self::send_address(reg, ctx.address(), 0);
                }
            }
            I2CEventInterrupt::AddressSent => {
                *ctx.state_mut() = State::AddressSend;
                // Clear condition by reading SR2
                reg.sr2.read();

                if ctx.is_read() {
                    // Do nothing...We don't have byte to read
                }
                if ctx.is_write() {
                    if let Some(btw) = ctx.byte_to_write() {
                        Self::send_byte(reg, btw);
                    } else {
                        Self::command_ended(reg, ctx);
                    }
                }
            }
            I2CEventInterrupt::DataByteTransferFinished => {
                if ctx.is_read() {
                    // All bytes expect last
                    if *ctx.state_mut() == State::AddressSend {
                        *ctx.state_mut() = State::ByteProcesseing;
                    }

                    let btr = Self::recv_byte(reg);
                    ctx.set_byte_to_read(btr);

                    if *ctx.state_mut() == State::LastByte {
                        hprintln!("I2C Event IT Read Last");
                        Self::command_ended(reg, ctx);
                        return;
                    }

                    if ctx.last_bytes_to_read() {
                        // Don't send ack for last byte
                        reg.cr1.modify(|_, w| w.ack().clear_bit().stop().set_bit());
                        *ctx.state_mut() = State::LastByte;
                    }
                }
                if ctx.is_write() {
                    *ctx.state_mut() = State::ByteProcesseing;
                    if let Some(btw) = ctx.byte_to_write() {
                        Self::send_byte(reg, btw);
                    } else {
                        Self::command_ended(reg, ctx);
                    }
                }
            }
            I2CEventInterrupt::Unknown => unreachable!(),
        }
    }

    #[inline(always)]
    fn handle_error_interrupt_impl(reg: &i2c1::RegisterBlock) {
        {
            NVIC::unpend(hal::interrupt::I2C1_ER)
        }
        hprintln!("I2C Error IT");
        let ctx = unsafe { &mut TRANSACTION };

        if let Err(e) = Self::check_and_clear_error_flags(reg) {
            *ctx.state_mut() = State::Fail(e);

            // Skip current transaction and start next if any
            if ctx.skip_transaction() {
                hprintln!("I2C Error IT Generate Start");
                Self::generate_start(reg)
            }
        }
    }

    pub fn enable_interupts(&self) {
        self.i2c
            .cr2
            .modify(|_, w| w.itevten().set_bit().iterren().set_bit());

        unsafe {
            NVIC::unmask(hal::interrupt::I2C1_EV);
            NVIC::unmask(hal::interrupt::I2C1_ER);
        }
    }

    fn generate_start(reg: &i2c1::RegisterBlock) {
        reg.cr1.modify(|_, w| w.start().set_bit().ack().set_bit());
    }

    fn generate_stop(reg: &i2c1::RegisterBlock) {
        // Send a STOP condition
        reg.cr1.modify(|_, w| w.ack().clear_bit().stop().set_bit());

        // Wait for STOP condition to transmit.
        while reg.cr1.read().stop().bit_is_set() {}
    }

    fn command_ended<const S: usize>(reg: &i2c1::RegisterBlock, ctx: &mut Transaction<S>) {
        if ctx.is_read() {
            // Read always last command

            // Wait for STOP condition to transmit.
            while reg.cr1.read().stop().bit_is_set() {}

            *ctx.state_mut() = State::Finished;

            if ctx.skip_transaction() {
                Self::generate_start(reg);
            } else {
                // Otherwise disable interupts
                NVIC::mask(hal::interrupt::I2C1_EV);
                NVIC::mask(hal::interrupt::I2C1_ER);
            }
        } else {
            *ctx.state_mut() = State::Finished;

            if ctx.next_command() {
                // Reset state and start new command
                *ctx.state_mut() = State::Begin;
                Self::generate_start(reg);
            } else {
                // We finished (NoOp found)
                Self::generate_stop(reg);

                // Check if have commands after NoOp
                // if yes, generate a new start
                if ctx.have_more_commands() {
                    Self::generate_start(reg);
                } else {
                    // Otherwise disable interupts
                    NVIC::mask(hal::interrupt::I2C1_EV);
                    NVIC::mask(hal::interrupt::I2C1_ER);
                }
            }
        }
    }

    fn send_address(reg: &i2c1::RegisterBlock, addr: u8, read: u32) {
        reg.dr
            .write(|w| unsafe { w.bits((u32::from(addr) << 1) + read) });
    }

    fn event_interupt_reason(reg: &i2c1::RegisterBlock) -> I2CEventInterrupt {
        let sr1 = reg.sr1.read();
        if sr1.sb().bit_is_set() {
            return I2CEventInterrupt::StartBitSent;
        }

        if sr1.addr().bit_is_set() {
            return I2CEventInterrupt::AddressSent;
        }

        if sr1.btf().bit_is_set() {
            return I2CEventInterrupt::DataByteTransferFinished;
        }

        return I2CEventInterrupt::Unknown;
    }

    fn send_byte(reg: &i2c1::RegisterBlock, byte: u8) {
        // Push out a byte of data
        reg.dr.write(|w| unsafe { w.bits(u32::from(byte)) });
    }

    fn recv_byte(reg: &i2c1::RegisterBlock) -> u8 {
        let value = reg.dr.read().bits() as u8;
        value
    }

    // Check is something is processing
    fn working(&self) -> bool {
        let ctx = unsafe { &mut TRANSACTION };
        ctx.commands.len() != 0
    }
}

impl<I2C: Instance, PINS> NonBlockingI2C for I2c<I2C, PINS> {
    fn write_read_async<'b>(
        &self,
        addr: u8,
        to_send: &'b [u8],
        to_recv: &'b mut [u8],
    ) -> Result<I2COperationFuture, Error> {
        let ctx = unsafe { &mut TRANSACTION };

        let static_send: &'static [u8] = unsafe { transmute(to_send) };
        let static_recv: &'static mut [u8] = unsafe { transmute(to_recv) };

        let write_cmd = Command::Write(addr, static_send);
        let read_cmd = Command::Read(addr, static_recv);

        critical_section::with(|_| {
            let gen_start = !self.working();

            match ctx.enqueue_commands([write_cmd, read_cmd]) {
                Ok(f) => {
                    if gen_start {
                        self.enable_interupts();
                        Self::generate_start(&self.i2c);
                    }
                    Ok(f)
                }
                Err(_) => Err(Error::Busy),
            }
        })
    }

    fn write_async<'b>(&self, addr: u8, to_send: &'b [u8]) -> Result<I2COperationFuture, Error> {
        let ctx = unsafe { &mut TRANSACTION };

        let static_send: &'static [u8] = unsafe { transmute(to_send) };

        let write_cmd = Command::Write(addr, static_send);

        critical_section::with(|_| {
            let gen_start = !self.working();

            match ctx.enqueue_commands([write_cmd]) {
                Ok(f) => {
                    if gen_start {
                        self.enable_interupts();
                        Self::generate_start(&self.i2c);
                    }
                    Ok(f)
                }
                Err(_) => Err(Error::Busy),
            }
        })
    }

    fn read_async<'b>(&self, addr: u8, to_recv: &'b mut [u8]) -> Result<I2COperationFuture, Error> {
        let ctx = unsafe { &mut TRANSACTION };

        let static_recv: &'static mut [u8] = unsafe { transmute(to_recv) };

        let read_cmd = Command::Read(addr, static_recv);

        critical_section::with(|_| {
            let gen_start = !self.working();

            match ctx.enqueue_commands([read_cmd]) {
                Ok(f) => {
                    if gen_start {
                        self.enable_interupts();
                        Self::generate_start(&self.i2c);
                    }
                    Ok(f)
                }
                Err(_) => Err(Error::Busy),
            }
        })
    }
}

unsafe impl<I2C: Instance, PINS> Send for I2c<I2C, PINS> {}
unsafe impl<I2C: Instance, PINS> Sync for I2c<I2C, PINS> {}

static mut TRANSACTION: Transaction<5> = Transaction::new();
