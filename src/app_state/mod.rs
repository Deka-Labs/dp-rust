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

    pub use super::AppSharedState;
    pub use super::AppStateHolder;
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

/// Basic primitives for drawing navigation hints
mod navigation;
use navigation::{NavigationDrawables, NavigationIcons};

/// Macro for using in [AppStateHolder] to run state method
macro_rules! run_state_func {
    ($holder: expr, $function: ident) => {
        match $holder.state {
            AppState::Clock => $holder.clock_state.$function(),
            AppState::Stopwatch => $holder.stopwatch_state.$function(),
            AppState::Timer => $holder.timer_state.$function(),
        }
    };

    ($holder: expr, $function: ident, $arg: expr) => {
        match $holder.state {
            AppState::Clock => $holder.clock_state.$function($arg),
            AppState::Stopwatch => $holder.stopwatch_state.$function($arg),
            AppState::Timer => $holder.timer_state.$function($arg),
        }
    };
}

/// Current app states
enum AppState {
    Clock,
    Timer,
    Stopwatch,
}

pub struct AppStateHolder {
    state: AppState,
    clock_state: ClockState,
    timer_state: TimerState,
    stopwatch_state: StopwatchState,
}

impl AppStateHolder {
    pub fn new(
        mut clock: ClockState,
        timer: TimerState,
        stopwatch: StopwatchState,
        shared_state: AppSharedState,
    ) -> Self {
        clock.enter(shared_state);

        Self {
            state: AppState::Clock,
            clock_state: clock,
            timer_state: timer,
            stopwatch_state: stopwatch,
        }
    }

    /// Switch to next state
    pub fn next(&mut self) {
        let shared_state = self.exit();
        self.state = match self.state {
            AppState::Clock => AppState::Stopwatch,
            AppState::Stopwatch => AppState::Timer,
            AppState::Timer => AppState::Clock,
        };
        self.enter(shared_state);
    }

    /// Switch to previous state
    pub fn prev(&mut self) {
        let shared_state = self.exit();
        self.state = match self.state {
            AppState::Clock => AppState::Timer,
            AppState::Stopwatch => AppState::Clock,
            AppState::Timer => AppState::Stopwatch,
        };
        self.enter(shared_state);
    }
}

/// Composite Drawable implementation
impl Drawable for AppStateHolder {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        run_state_func!(self, draw, target)
    }
}

/// Composite AppStateTrait
impl AppStateTrait for AppStateHolder {
    fn enter(&mut self, state: AppSharedState) {
        run_state_func!(self, enter, state)
    }

    fn exit(&mut self) -> AppSharedState {
        run_state_func!(self, exit)
    }

    fn state(&self) -> &AppSharedState {
        run_state_func!(self, state)
    }

    fn tick(&self) {
        run_state_func!(self, tick)
    }

    fn handle_input<J: Joystick>(&self, joystick: &J) {
        run_state_func!(self, handle_input, joystick)
    }
}

/// Shared between all states
pub struct AppSharedState {
    header_style: MonoTextStyle<'static, BinaryColor>,
    content_style: MonoTextStyle<'static, BinaryColor>,
    small_text_style: MonoTextStyle<'static, BinaryColor>,

    navigation_icons: NavigationDrawables,
}

impl Default for AppSharedState {
    fn default() -> Self {
        use embedded_graphics::mono_font::iso_8859_5::{FONT_6X10, FONT_9X15_BOLD};

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
