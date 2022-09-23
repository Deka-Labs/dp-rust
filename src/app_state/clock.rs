use core::sync::atomic::{AtomicBool, Ordering};

use atomic_enum::atomic_enum;
use chrono::{prelude::*, Duration};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Triangle,
    text::{Alignment, Text},
};

use spin::lock_api::RwLock;

use crate::{ds3231::DS3231, format::format_time, i2c::I2c1Handle, joystick::Joystick};

use super::{navigation::NavigationIcons, AppSharedState, AppStateTrait};

#[atomic_enum]
enum EditField {
    Hours,
    Minutes,
}

pub struct ClockState {
    state: Option<AppSharedState>,

    rtc: DS3231<I2c1Handle>,
    display_time: RwLock<DateTime<Utc>>,

    edit_mode: AtomicBool,
    edit_field: AtomicEditField,
    edit_speed: f32,
}

impl ClockState {
    pub fn new(rtc: &DS3231<I2c1Handle>) -> Self {
        Self {
            state: None,
            rtc: rtc.clone(),
            display_time: RwLock::new(Default::default()),

            edit_mode: AtomicBool::new(false),
            edit_field: AtomicEditField::new(EditField::Hours),
            edit_speed: 1.0,
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
                    let mut dt = self.display_time.write();
                    // After apply we want to start count seconds over
                    *dt = dt.with_second(0).unwrap();
                }

                _ => {}
            }
        }
    }

    /// In edit mode navigation unavaiable
    fn handle_input_edit_mode<J: Joystick>(&self, j: &J) {
        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                // Up pressed
                Up => self.edit_field_add(),
                // Down pressed
                Down => self.edit_field_sub(),
                // Left pressed
                Left => self.edit_field_prev(),
                // Right pressed
                Right => self.edit_field_next(),
                Center => {
                    // Set time and exit form edit mode
                    self.rtc.set_time(*self.display_time.read()).unwrap();
                    self.edit_mode.store(false, Ordering::Release);
                }
            }
        }
    }

    /// Add rounded `edit_speed` value to current edit value
    fn edit_field_add(&self) {
        match self.edit_field.load(Ordering::Relaxed) {
            EditField::Hours => {
                *self.display_time.write() += Duration::hours(self.edit_speed as i64)
            }
            EditField::Minutes => {
                *self.display_time.write() += Duration::minutes(self.edit_speed as i64)
            }
        }
    }

    /// Substract rounded `edit_speed` value to current edit value
    fn edit_field_sub(&self) {
        match self.edit_field.load(Ordering::Relaxed) {
            EditField::Hours => {
                *self.display_time.write() -= Duration::hours(self.edit_speed as i64)
            }
            EditField::Minutes => {
                *self.display_time.write() -= Duration::minutes(self.edit_speed as i64)
            }
        }
    }

    /// Switch to next edit field
    fn edit_field_next(&self) {
        let new_field = match self.edit_field.load(Ordering::Acquire) {
            EditField::Hours => EditField::Minutes,
            EditField::Minutes => EditField::Hours,
        };

        self.edit_field.store(new_field, Ordering::Release);
    }

    /// Switch to previous edit field
    fn edit_field_prev(&self) {
        // will work only with 2 fields
        self.edit_field_next();
    }
}

impl AppStateTrait for ClockState {
    fn enter(&mut self, state: AppSharedState) {
        assert!(self.state.is_none());
        self.state = Some(state);

        // Get time from RTC module
        *self.display_time.write() = self.rtc.update_time().unwrap();
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
            *self.display_time.write() += Duration::seconds(1);
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
            let arrow = Triangle::new(
                Point { x: 0, y: -4 },
                Point { x: -2, y: 3 },
                Point { x: 2, y: 3 },
            )
            .into_styled(self.state().primitive_style);

            let field = self.edit_field.load(Ordering::Relaxed);
            let height = 40;
            let pos = match field {
                EditField::Hours => Point { x: 36, y: height },
                EditField::Minutes => Point { x: 64, y: height },
            };

            arrow.translate(pos).draw(target)?;
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
        let mut buf = [0_u8; 32];

        let time = self.display_time.read();
        let time_str = format_time(&mut buf, &time).unwrap();
        Text::with_alignment(
            time_str,
            Point { x: 64, y: 32 },
            self.state().content_style,
            Alignment::Center,
        )
        .draw(target)?;

        Ok(())
    }
}
