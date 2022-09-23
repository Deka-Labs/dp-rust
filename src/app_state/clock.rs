use chrono::{prelude::*, Duration};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};

use spin::lock_api::RwLock;

use crate::{ds3231::DS3231, format::format_time, i2c::I2c1Handle, joystick::Joystick};

use super::{navigation::NavigationIcons, AppSharedState, AppStateTrait};

pub struct ClockState {
    state: Option<AppSharedState>,

    rtc: DS3231<I2c1Handle>,
    display_time: RwLock<DateTime<Utc>>,
}

impl ClockState {
    pub fn new(rtc: &DS3231<I2c1Handle>) -> Self {
        Self {
            state: None,
            rtc: rtc.clone(),
            display_time: RwLock::new(Default::default()),
        }
    }
}

impl AppStateTrait for ClockState {
    fn enter(&mut self, state: AppSharedState) {
        assert!(self.state.is_none());
        self.state = Some(state);

        // Get time from RTC module
        self.rtc.update_time().unwrap();
        *self.display_time.write() = self.rtc.time().clone();
    }

    fn exit(&mut self) -> AppSharedState {
        self.state.take().expect("exit called without enter")
    }

    fn state(&self) -> &AppSharedState {
        self.state.as_ref().unwrap()
    }

    fn tick(&self) {
        // On tick increment time
        *self.display_time.write() += Duration::seconds(1);
    }

    fn handle_input<J: Joystick>(&self, j: &J) {
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

                _ => {}
            }
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
        self.draw_navigation(target)?;

        // Draw UI hints
        let state = self.state();
        state.navigation_icons.draw_icon_and_text(
            target,
            NavigationIcons::Center,
            Point::new(20, 60),
            Text::new("Изменить", Default::default(), state.small_text_style),
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
