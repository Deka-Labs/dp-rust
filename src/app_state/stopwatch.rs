use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use stm32f4xx_hal::pac::TIM3;

use crate::{joystick::Joystick, stopwatchtimer::StopwatchTimer};

use super::{AppSharedState, AppStateTrait};

pub struct StopwatchState {
    state: Option<AppSharedState>,

    stopwatch: &'static StopwatchTimer<TIM3>,
}

impl StopwatchState {
    pub fn new(timer_ref: &'static StopwatchTimer<TIM3>) -> Self {
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
                        self.stopwatch.stop();
                    } else {
                        self.stopwatch.start();
                    }
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
        self.draw_header(target, "СЕКУНДОМЕТР")?;

        let mut buf = [0_u8; 32];
        let time_str = crate::format::format_u32(&mut buf, self.stopwatch.elapsed()).unwrap();

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
