use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use super::{AppSharedState, AppStateTrait};

pub struct StopwatchState {
    state: Option<AppSharedState>,
}

impl StopwatchState {
    pub fn new() -> Self {
        Self { state: None }
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
}

impl Drawable for StopwatchState {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.draw_header(target, "СЕКУНДОМЕТР")?;

        Ok(())
    }
}
