use hal::gpio::{Input, Pin};

pub trait Button {
    fn pressed(&self) -> bool;
}

pub struct ButtonPullUp<PIN> {
    pin: PIN,
}

impl<const P: char, const N: u8> ButtonPullUp<Pin<P, N, Input>> {
    pub fn new(pin: Pin<P, N, Input>) -> Self {
        let p = pin.internal_pull_up(true);
        Self { pin: p }
    }
}

impl<const P: char, const N: u8> Button for ButtonPullUp<Pin<P, N, Input>> {
    fn pressed(&self) -> bool {
        self.pin.is_low()
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
}
