use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use super::{AppSharedState, AppStateTrait};

pub struct TimerState {
    state: Option<AppSharedState>,
}

impl TimerState {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl AppStateTrait for TimerState {
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
}

impl Drawable for TimerState {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.draw_header(target, "ТАЙМЕР")?;

        Ok(())
    }
}
