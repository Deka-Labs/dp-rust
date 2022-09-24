use core::cell::RefCell;

use hal::gpio::PA7;
use hal::pac::TIM3;
use hal::prelude::*;
use hal::timer::PwmExt;
use stm32f4xx_hal::rcc::Clocks;
use stm32f4xx_hal::timer::PwmChannel;

pub struct Buzzer {
    ch: RefCell<PwmChannel<TIM3, 1>>,
}

impl Buzzer {
    pub fn new(timer: TIM3, pin: PA7, clocks: &Clocks) -> Self {
        let pwm = timer.pwm_hz(pin.into_alternate(), 1.kHz(), clocks);
        let mut ch = pwm.split();
        let max_duty = ch.get_max_duty();
        ch.set_duty(max_duty / 10);

        Self {
            ch: RefCell::new(ch),
        }
    }

    pub fn enable(&self) {
        self.ch.borrow_mut().enable();
    }

    pub fn disable(&self) {
        self.ch.borrow_mut().disable();
    }
}
