use hal::syscfg::SysCfg;
use stm32f4xx_hal::{gpio::Input, pac::EXTI, prelude::_stm32f4xx_hal_gpio_ExtiPin};

use crate::hal::gpio::Pin;

pub trait Button {
    fn pressed(&self) -> bool;
    fn clear_interrupts(&mut self);
}

pub struct ButtonPullUp<PIN> {
    pin: PIN,
}

impl<const P: char, const N: u8> ButtonPullUp<Pin<P, N, Input>> {
    pub fn new(pin: Pin<P, N, Input>, _exti: &mut EXTI, _syscfg: &mut SysCfg) -> Self {
        let p = pin.internal_pull_up(true);
        // p.make_interrupt_source(syscfg);
        // p.enable_interrupt(exti);
        // p.trigger_on_edge(exti, Edge::Falling);

        // let it = p.interrupt();
        // // Safe because 1-time calling on init
        // unsafe {
        //     NVIC::unmask(it);
        // }

        Self { pin: p }
    }
}

impl<const P: char, const N: u8> Button for ButtonPullUp<Pin<P, N, Input>> {
    fn pressed(&self) -> bool {
        self.pin.is_low()
    }

    fn clear_interrupts(&mut self) {
        self.pin.clear_interrupt_pending_bit();
    }
}
pub struct AccessoryShieldJoystick<U, D, L, R, C>
where
    U: Button,
    D: Button,
    L: Button,
    R: Button,
    C: Button,
{
    pub up: U,
    pub down: D,
    pub left: L,
    pub right: R,
    pub center: C,
}

impl<U, D, L, R, C> AccessoryShieldJoystick<U, D, L, R, C>
where
    U: Button,
    D: Button,
    L: Button,
    R: Button,
    C: Button,
{
    pub fn new(up: U, down: D, left: L, right: R, center: C) -> Self {
        AccessoryShieldJoystick {
            up,
            down,
            left,
            right,
            center,
        }
    }

    pub fn clear_interrupts(&mut self) {
        self.up.clear_interrupts();
        self.down.clear_interrupts();
        self.left.clear_interrupts();
        self.right.clear_interrupts();
        self.center.clear_interrupts();
    }
}
