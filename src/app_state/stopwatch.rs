use core::fmt::Write;

use chrono::Duration;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use heapless::String;

use crate::app::StopwatchTimer;
use crate::joystick::Joystick;

use super::{navigation::NavigationIcons, AppSharedState, AppStateTrait};

pub struct StopwatchState {
    state: Option<AppSharedState>,

    stopwatch: &'static StopwatchTimer,
}

impl StopwatchState {
    pub fn new(timer_ref: &'static StopwatchTimer) -> Self {
        Self {
            state: None,
            stopwatch: timer_ref,
        }
    }
}

impl AppStateTrait for StopwatchState {
    fn enter(&mut self, state: AppSharedState) {
        assert!(self.state.is_none());
        self.state = Some(state);
    }

    fn exit(&mut self) -> AppSharedState {
        self.state.take().expect("exit called without enter")
    }

    fn state(&self) -> &AppSharedState {
        self.state.as_ref().unwrap()
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
                Center => {
                    if self.stopwatch.started() {
                        self.stopwatch.pause();
                    } else {
                        self.stopwatch.start();
                    }
                }
                Down => {
                    self.stopwatch.stop();
                }

                _ => {}
            }
        }
    }
}

impl Drawable for StopwatchState {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.draw_header(target, "СЕКУНДОМЕР")?;
        self.draw_navigation(target)?;

        // Draw UI help
        let center_button_hint = if self.stopwatch.started() {
            "Пауза"
        } else {
            "Старт"
        };

        let state = self.state();
        state.navigation_icons.draw_icon_and_text(
            target,
            NavigationIcons::Center,
            Point::new(20, 46),
            Text::new(
                center_button_hint,
                Default::default(),
                state.small_text_style,
            ),
        )?;

        state.navigation_icons.draw_icon_and_text(
            target,
            NavigationIcons::Down,
            Point::new(20, 56),
            Text::new("Стоп и сброс", Default::default(), state.small_text_style),
        )?;

        // Draw elapsed time
        let mut buf: String<32> = Default::default();
        let elapsed = Duration::milliseconds(self.stopwatch.elapsed() as i64);
        let hours = elapsed.num_hours();
        let minutes = elapsed.num_minutes() - 60 * hours;
        let seconds = elapsed.num_seconds() - 60 * minutes - 60 * 60 * hours;

        // ms / 100 - to display only we supported
        let hecto_ms = (elapsed.num_milliseconds()
            - 1000 * seconds
            - 60 * 1000 * minutes
            - 60 * 60 * 1000 * hours)
            / 100;

        write!(
            &mut buf,
            "{:}:{:02}:{:02}.{:01}",
            hours, minutes, seconds, hecto_ms
        )
        .unwrap();

        Text::with_alignment(
            &buf,
            Point { x: 64, y: 32 },
            self.state().content_style,
            Alignment::Center,
        )
        .draw(target)?;

        Ok(())
    }
}
