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

#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum JoystickButton {
    Up = 0,
    Down,
    Left,
    Right,
    Center,
}

pub trait Joystick {
    /// Current joystick position
    fn position(&self) -> &Option<JoystickButton>;

    /// Is current position just clicked
    fn clicked(&self) -> bool;

    /// Is joystick just unpressed
    fn just_unpressed(&self) -> bool;

    /// How many update intervals passed from pressing
    fn hold_time(&self) -> u32;

    /// Update joystick status
    fn update(&mut self);
}

pub struct AccessoryShieldJoystick<U, D, L, R, C>
where
    U: Button,
    D: Button,
    L: Button,
    R: Button,
    C: Button,
{
    up: U,
    down: D,
    left: L,
    right: R,
    center: C,

    prev_position: Option<JoystickButton>,
    position: Option<JoystickButton>,
    time_wo_change: u32,
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

            prev_position: None,
            position: None,

            time_wo_change: 0,
        }
    }
}

impl<U, D, L, R, C> Joystick for AccessoryShieldJoystick<U, D, L, R, C>
where
    U: Button,
    D: Button,
    L: Button,
    R: Button,
    C: Button,
{
    fn position(&self) -> &Option<JoystickButton> {
        &self.position
    }

    fn clicked(&self) -> bool {
        self.prev_position.is_none() && self.position.is_some()
    }

    fn just_unpressed(&self) -> bool {
        self.prev_position.is_some() && self.position.is_none()
    }

    fn hold_time(&self) -> u32 {
        self.time_wo_change
    }

    fn update(&mut self) {
        self.prev_position = self.position.take();

        use JoystickButton::*;

        if self.up.pressed() {
            self.position = Some(Up);
        } else if self.down.pressed() {
            self.position = Some(Down);
        } else if self.left.pressed() {
            self.position = Some(Left);
        } else if self.right.pressed() {
            self.position = Some(Right);
        } else if self.center.pressed() {
            self.position = Some(Center);
        } else {
            self.position = None;
        }

        if self.prev_position != self.position {
            self.time_wo_change = 0;
        } else {
            self.time_wo_change += 1;
        }
    }
}
