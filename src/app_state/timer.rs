use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use atomic_enum::atomic_enum;
use chrono::Duration;
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use heapless::String;

use crate::app::CountdownTimer;
use crate::joystick::Joystick;
use crate::speedchanger::SpeedChanger;

use super::navigation::NavigationIcons;
use super::{AppSharedState, AppStateTrait};

const SPEED_STEPS: u32 = 8;
const ACCELERAION_TICKS: u32 = 10;
const MAX_TIMER_COUNTDOWN: u32 = 60 * 60 * 99 + 60 * 59 + 59; // 99 hours, 59 mins, 59 secs

#[atomic_enum]
#[derive(PartialEq)]
enum TimerInternalState {
    /// Timer not started
    TimerEnd,
    /// Edit countdown
    Edit,
    /// Timers started, display countdown
    TimerStarted,
}

#[atomic_enum]
enum EditField {
    Hours,
    Minutes,
    Seconds,
}

impl AtomicEditField {
    fn next(&self) {
        let new_field = match self.load(Ordering::Acquire) {
            EditField::Hours => EditField::Minutes,
            EditField::Minutes => EditField::Seconds,
            EditField::Seconds => EditField::Hours,
        };

        self.store(new_field, Ordering::Release);
    }

    fn prev(&self) {
        let new_field = match self.load(Ordering::Acquire) {
            EditField::Hours => EditField::Seconds,
            EditField::Minutes => EditField::Hours,
            EditField::Seconds => EditField::Minutes,
        };

        self.store(new_field, Ordering::Release);
    }

    fn edit_amount(&self) -> u32 {
        match self.load(Ordering::Relaxed) {
            EditField::Hours => 60 * 60,
            EditField::Minutes => 60,
            EditField::Seconds => 1,
        }
    }

    fn countdown_add(&self, c: &AtomicU32) {
        let to_add = self.edit_amount();

        let mut counter = c.load(Ordering::Acquire);
        let diff = MAX_TIMER_COUNTDOWN - counter;
        if diff > to_add {
            counter += to_add;
        } else {
            counter = MAX_TIMER_COUNTDOWN;
        }
        c.store(counter, Ordering::Release);
    }

    fn countdown_sub(&self, c: &AtomicU32) {
        let to_sub = self.edit_amount();

        let mut counter = c.load(Ordering::Acquire);
        if counter > to_sub {
            counter -= to_sub;
        } else {
            counter = 0;
        }
        c.store(counter, Ordering::Release);
    }
}

pub struct TimerState {
    state: Option<AppSharedState>,
    timer: &'static CountdownTimer,
    internal_state: AtomicTimerInternalState,

    countdown_selected: AtomicU32,
    edit_field: AtomicEditField,
    edit_speed: SpeedChanger<SPEED_STEPS>,
    edit_acceleration: SpeedChanger<ACCELERAION_TICKS>,
}

impl TimerState {
    pub fn new(timer: &'static CountdownTimer) -> Self {
        let mut start_int_state = TimerInternalState::TimerEnd;
        if timer.started() {
            start_int_state = TimerInternalState::TimerStarted;
        }

        Self {
            state: None,

            timer,

            internal_state: AtomicTimerInternalState::new(start_int_state),
            countdown_selected: AtomicU32::new(0),
            edit_field: AtomicEditField::new(EditField::Seconds),
            edit_speed: Default::default(),
            edit_acceleration: Default::default(),
        }
    }

    pub fn handle_input_end<J: Joystick>(&self, j: &J) {
        if j.position().is_none() {
            return;
        }

        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                Left => {
                    crate::app::change_state::spawn(false).ok();
                }
                Right => {
                    crate::app::change_state::spawn(true).ok();
                }
                Center => self
                    .internal_state
                    .store(TimerInternalState::Edit, Ordering::Relaxed),

                _ => {}
            }
        }
    }

    pub fn handle_input_edit<J: Joystick>(&self, j: &J) {
        const HOLD_DURATION_TICK: u32 = 2;

        if j.position().is_none() {
            return;
        }

        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                // Up pressed
                Up => self.edit_field.countdown_add(&self.countdown_selected),
                // Down pressed
                Down => self.edit_field.countdown_sub(&self.countdown_selected),
                // Left pressed
                Left => self.edit_field.prev(),
                // Right pressed
                Right => self.edit_field.next(),
                Center => {
                    self.timer
                        .start(self.countdown_selected.load(Ordering::Relaxed));

                    self.internal_state
                        .store(TimerInternalState::TimerStarted, Ordering::Relaxed);
                }
            }
        }

        if j.hold_time() > HOLD_DURATION_TICK {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;
            self.edit_speed.execute(|| {
                match pos {
                    // Up pressed
                    Up => self.edit_field.countdown_add(&self.countdown_selected),
                    // Down pressed
                    Down => self.edit_field.countdown_sub(&self.countdown_selected),
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

    pub fn handle_input_started<J: Joystick>(&self, j: &J) {
        if j.position().is_none() {
            return;
        }

        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                Left => {
                    crate::app::change_state::spawn(false).ok();
                }
                Right => {
                    crate::app::change_state::spawn(true).ok();
                }
                Center => {
                    self.timer.stop();
                    self.internal_state
                        .store(TimerInternalState::TimerEnd, Ordering::Relaxed);
                }

                _ => {}
            }
        }
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

    fn handle_input<J: Joystick>(&self, j: &J) {
        match self.internal_state.load(Ordering::Relaxed) {
            TimerInternalState::TimerEnd => self.handle_input_end(j),
            TimerInternalState::Edit => self.handle_input_edit(j),
            TimerInternalState::TimerStarted => self.handle_input_started(j),
        }
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

        let int_state = self.internal_state.load(Ordering::Relaxed);

        // Draw UI hints
        let center_button_hint = match int_state {
            TimerInternalState::TimerEnd => "Задать",
            TimerInternalState::Edit => "Запуск",
            TimerInternalState::TimerStarted => "Стоп",
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

        if int_state != TimerInternalState::Edit {
            self.draw_navigation(target)?;
        }

        // Draw current countdown
        let countdown_to_draw = match int_state {
            TimerInternalState::TimerEnd => 0,
            TimerInternalState::Edit => self.countdown_selected.load(Ordering::Relaxed),
            TimerInternalState::TimerStarted => self.timer.countdown(),
        };

        let mut buf: String<32> = Default::default();
        let elapsed = Duration::seconds(countdown_to_draw as i64);
        let hours = elapsed.num_hours();
        let minutes = elapsed.num_minutes() - 60 * hours;
        let seconds = elapsed.num_seconds() - 60 * minutes - 60 * 60 * hours;

        write!(&mut buf, "{:02}:{:02}:{:02}", hours, minutes, seconds).unwrap();

        Text::with_alignment(
            &buf,
            Point { x: 64, y: 34 },
            self.state().content_style,
            Alignment::Center,
        )
        .draw(target)?;

        // Draw selector
        if int_state == TimerInternalState::Edit {
            let y_above = 19;
            let y_below = 40;

            let field = self.edit_field.load(Ordering::Relaxed);
            let x_pos = match field {
                EditField::Hours => 36,
                EditField::Minutes => 64,
                EditField::Seconds => 92,
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
        }

        Ok(())
    }
}
