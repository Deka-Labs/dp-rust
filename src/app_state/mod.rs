use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::PrimitiveStyleBuilder,
    text::{Alignment, Text},
};

use crate::joystick::Joystick;

pub mod prelude {
    pub use super::clock::ClockState;
    pub use super::stopwatch::StopwatchState;
    pub use super::timer::TimerState;

    pub use super::switch::SwitchState;
    pub use super::AppSharedState;
    pub use super::AppState;
    pub use super::AppStateTrait;
}

/// Clock state
mod clock;
use clock::ClockState;

/// Stopwatch state
mod stopwatch;
use stopwatch::StopwatchState;

/// Timer state
mod timer;
use timer::TimerState;

/// Switch trait for specify mode switching
mod switch;
use switch::SwitchState;

/// Basic primitives for drawing navigation hints
mod navigation;
use navigation::{NavigationDrawables, NavigationIcons};

/// Current app state holder
pub enum AppState {
    Clock(ClockState),
    Timer(TimerState),
    Stopwatch(StopwatchState),
}

/// Allow switch to clock state
impl SwitchState<ClockState> for AppState {
    fn switch(&mut self, new_state: ClockState) {
        let state = self.exit();
        *self = AppState::Clock(new_state);
        self.enter(state)
    }
}

/// Allow switch to timer state
impl SwitchState<TimerState> for AppState {
    fn switch(&mut self, new_state: TimerState) {
        let state = self.exit();
        *self = AppState::Timer(new_state);
        self.enter(state)
    }
}

/// Allow switch to stopwatch state
impl SwitchState<StopwatchState> for AppState {
    fn switch(&mut self, new_state: StopwatchState) {
        let state = self.exit();
        *self = AppState::Stopwatch(new_state);
        self.enter(state)
    }
}

/// Composite Drawable implementation
impl Drawable for AppState {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        match self {
            AppState::Clock(s) => s.draw(target),
            AppState::Stopwatch(s) => s.draw(target),
            AppState::Timer(s) => s.draw(target),
        }
    }
}

/// Composite AppStateTrait
impl AppStateTrait for AppState {
    fn enter(&mut self, state: AppSharedState) {
        match self {
            AppState::Clock(s) => s.enter(state),
            AppState::Stopwatch(s) => s.enter(state),
            AppState::Timer(s) => s.enter(state),
        }
    }

    fn exit(&mut self) -> AppSharedState {
        match self {
            AppState::Clock(s) => s.exit(),
            AppState::Stopwatch(s) => s.exit(),
            AppState::Timer(s) => s.exit(),
        }
    }

    fn state(&self) -> &AppSharedState {
        match self {
            AppState::Clock(s) => s.state(),
            AppState::Stopwatch(s) => s.state(),
            AppState::Timer(s) => s.state(),
        }
    }

    fn tick(&self) {
        match self {
            AppState::Clock(s) => s.tick(),
            AppState::Stopwatch(s) => s.tick(),
            AppState::Timer(s) => s.tick(),
        }
    }

    fn handle_input<J: Joystick>(&self, joystick: &J) {
        match self {
            AppState::Clock(s) => s.handle_input(joystick),
            AppState::Stopwatch(s) => s.handle_input(joystick),
            AppState::Timer(s) => s.handle_input(joystick),
        }
    }
}

/// Shared between all states
pub struct AppSharedState {
    header_style: MonoTextStyle<'static, BinaryColor>,
    content_style: MonoTextStyle<'static, BinaryColor>,
    small_text_style: MonoTextStyle<'static, BinaryColor>,

    navigation_icons: NavigationDrawables,
}

impl AppSharedState {
    pub fn new() -> Self {
        use embedded_graphics::mono_font::iso_8859_5::FONT_6X10;
        use embedded_graphics::mono_font::iso_8859_5::FONT_9X15_BOLD;

        let primitive_style = PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(BinaryColor::On)
            .fill_color(BinaryColor::Off)
            .build();

        Self {
            header_style: MonoTextStyleBuilder::new()
                .font(&FONT_9X15_BOLD)
                .text_color(BinaryColor::On)
                .build(),
            content_style: MonoTextStyleBuilder::new()
                .font(&FONT_9X15_BOLD)
                .text_color(BinaryColor::On)
                .build(),
            small_text_style: MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(BinaryColor::On)
                .build(),

            navigation_icons: NavigationDrawables::new(&primitive_style),
        }
    }
}

pub trait AppStateTrait: Drawable<Color = BinaryColor, Output = ()> {
    /// enters in application state with specified shared state
    fn enter(&mut self, state: AppSharedState);
    /// exit from state and return shared state. Will block if some task in progress so should be in low priority task
    fn exit(&mut self) -> AppSharedState;

    /// Shared state getter
    fn state(&self) -> &AppSharedState;

    /// tick function called each second. By default do nothing
    /// This is high priority function
    fn tick(&self) {}

    fn handle_input<J: Joystick>(&self, joystick: &J);

    /// Draw header at top of display
    fn draw_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
        header: &str,
    ) -> Result<(), D::Error> {
        Text::with_alignment(
            header,
            Point { x: 64, y: 10 },
            self.state().header_style,
            Alignment::Center,
        )
        .draw(target)?;

        Ok(())
    }

    /// Draw 2 triangles to indicate mode switch posibility
    fn draw_navigation<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let targets_box = target.bounding_box();
        let width = targets_box.bottom_right().unwrap().x;

        let state = self.state();

        state
            .navigation_icons
            .draw_icon(target, NavigationIcons::Left, Point::new(4, 32))?;

        state.navigation_icons.draw_icon(
            target,
            NavigationIcons::Right,
            Point::new(width - 4, 32),
        )?;

        Ok(())
    }
}
