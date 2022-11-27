use core::cell::Cell;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use atomic_enum::atomic_enum;
use chrono::{prelude::*, Duration};
use critical_section::Mutex;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use heapless::String;

use crate::{ds3231::DS3231, i2c::I2c1Handle, joystick::Joystick, speedchanger::SpeedChanger};

use super::{navigation::NavigationIcons, AppSharedState, AppStateTrait};

const SPEED_STEPS: u32 = 8;
const ACCELERAION_TICKS: u32 = 10;

#[atomic_enum]
enum EditField {
    Hours,
    Minutes,
}

impl AtomicEditField {
    fn next(&self) {
        let new_field = match self.load(Ordering::Acquire) {
            EditField::Hours => EditField::Minutes,
            EditField::Minutes => EditField::Hours,
        };

        self.store(new_field, Ordering::Release);
    }

    fn prev(&self) {
        // will work only with 2 fields
        self.next();
    }

    fn edit_duration(&self) -> Duration {
        match self.load(Ordering::Relaxed) {
            EditField::Hours => Duration::hours(1),
            EditField::Minutes => Duration::minutes(1),
        }
    }

    fn time_add(&self, time: &Mutex<Cell<DateTime<Utc>>>) {
        let edit_amount = self.edit_duration();
        critical_section::with(|cs| {
            let dt = time.borrow(cs);
            dt.set(dt.get() + edit_amount);
        });
    }

    fn time_sub(&self, time: &Mutex<Cell<DateTime<Utc>>>) {
        let edit_amount = self.edit_duration();
        critical_section::with(|cs| {
            let dt = time.borrow(cs);
            dt.set(dt.get() - edit_amount);
        });
    }
}

pub struct ClockState {
    state: Option<AppSharedState>,

    rtc: DS3231<I2c1Handle>,
    display_time: Mutex<Cell<DateTime<Utc>>>,

    edit_mode: AtomicBool,
    edit_field: AtomicEditField,
    edit_speed: SpeedChanger<SPEED_STEPS>,
    edit_acceleration: SpeedChanger<ACCELERAION_TICKS>,
}

impl ClockState {
    pub fn new(rtc: DS3231<I2c1Handle>) -> Self {
        Self {
            state: None,
            rtc,
            display_time: Mutex::new(Cell::new(Default::default())),

            edit_mode: AtomicBool::new(false),
            edit_field: AtomicEditField::new(EditField::Minutes),
            edit_speed: Default::default(),
            edit_acceleration: Default::default(),
        }
    }

    /// In normal mode allow navigation and mode switch
    fn handle_input_normal_mode<J: Joystick>(&self, j: &J) {
        if j.clicked() && j.position().is_some() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                Left => {
                    // Request from app mode switch
                    // It will run after exit from this function due low priority
                    crate::app::change_state::spawn(false).ok();
                }
                Right => {
                    crate::app::change_state::spawn(true).ok();
                }
                Center => {
                    self.edit_mode.store(true, Ordering::Release);
                    critical_section::with(|cs| {
                        let dt = self.display_time.borrow(cs);
                        // After apply we want to start count seconds over
                        dt.set(dt.get().with_second(0).unwrap());
                    })
                }

                _ => {}
            }
        }
    }

    /// In edit mode navigation unavaiable
    fn handle_input_edit_mode<J: Joystick>(&self, j: &J) {
        const HOLD_DURATION_TICK: u32 = 10;

        if j.position().is_none() {
            return;
        }

        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                // Up pressed
                Up => self.edit_field.time_add(&self.display_time),
                // Down pressed
                Down => self.edit_field.time_sub(&self.display_time),
                // Left pressed
                Left => self.edit_field.prev(),
                // Right pressed
                Right => self.edit_field.next(),
                Center => {
                    // Set time and exit form edit mode
                    critical_section::with(|cs| {
                        let dt = self.display_time.borrow(cs);
                        self.rtc.set_time(dt.get()).unwrap();
                    });
                    self.edit_mode.store(false, Ordering::Release);
                }
            }
        }

        if j.hold_time() > HOLD_DURATION_TICK {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;
            self.edit_speed.execute(|| {
                match pos {
                    // Up pressed
                    Up => self.edit_field.time_add(&self.display_time),
                    // Down pressed
                    Down => self.edit_field.time_sub(&self.display_time),
                    _ => {}
                }
            });

            self.edit_acceleration.execute(|| {
                self.edit_speed.decrement_max_div();
            });
        } else {
            self.edit_speed.reset();
            self.edit_acceleration.reset();
        }
    }
}

impl AppStateTrait for ClockState {
    fn enter(&mut self, state: AppSharedState) {
        assert!(self.state.is_none());
        self.state = Some(state);

        // Get time from RTC module
        let time = self.rtc.update_time().unwrap();
        critical_section::with(|cs| {
            self.display_time.borrow(cs).set(time);
        });
    }

    fn exit(&mut self) -> AppSharedState {
        self.state.take().expect("exit called without enter")
    }

    fn state(&self) -> &AppSharedState {
        self.state.as_ref().unwrap()
    }

    fn tick(&self) {
        // On tick increment time if not in edit mode
        if !self.edit_mode.load(Ordering::Relaxed) {
            critical_section::with(|cs| {
                let dt = self.display_time.borrow(cs);
                dt.set(dt.get() + Duration::seconds(1))
            });
        }
    }

    fn handle_input<J: Joystick>(&self, j: &J) {
        if self.edit_mode.load(Ordering::Acquire) {
            self.handle_input_edit_mode(j)
        } else {
            self.handle_input_normal_mode(j)
        }
    }
}

impl Drawable for ClockState {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.draw_header(target, "ЧАСЫ")?;

        let is_edit = self.edit_mode.load(Ordering::Relaxed);

        // Draw UI hints

        if is_edit {
            let y_above = 19;
            let y_below = 40;

            let field = self.edit_field.load(Ordering::Relaxed);
            let x_pos = match field {
                EditField::Hours => 36,
                EditField::Minutes => 64,
            };

            self.state().navigation_icons.draw_icon(
                target,
                NavigationIcons::Up,
                Point {
                    x: x_pos,
                    y: y_above,
                },
            )?;

            self.state().navigation_icons.draw_icon(
                target,
                NavigationIcons::Down,
                Point {
                    x: x_pos,
                    y: y_below,
                },
            )?;
        } else {
            self.draw_navigation(target)?;
        }

        let center_button_hint = if is_edit {
            "Применить"
        } else {
            "Изменить"
        };

        let state = self.state();
        state.navigation_icons.draw_icon_and_text(
            target,
            NavigationIcons::Center,
            Point::new(20, 56),
            Text::new(
                center_button_hint,
                Default::default(),
                state.small_text_style,
            ),
        )?;

        // Draw time
        let mut buf: String<32> = Default::default();
        let time = critical_section::with(|cs| self.display_time.borrow(cs).get());

        write!(
            &mut buf,
            "{:02}:{:02}:{:02}",
            time.hour(),
            time.minute(),
            time.second()
        )
        .unwrap();

        Text::with_alignment(
            &buf,
            Point { x: 64, y: 34 },
            self.state().content_style,
            Alignment::Center,
        )
        .draw(target)?;

        Ok(())
    }
}
