use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use atomic_enum::atomic_enum;
use chrono::Duration;
use embedded_graphics::primitives::Triangle;
use embedded_graphics::text::{Alignment, Text};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use heapless::String;

use crate::app::CountdownTimer;
use crate::joystick::Joystick;

use super::navigation::NavigationIcons;
use super::{AppSharedState, AppStateTrait};

const SPEED_STEPS: u32 = 8;
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

pub struct TimerState {
    state: Option<AppSharedState>,
    timer: &'static CountdownTimer,
    internal_state: AtomicTimerInternalState,

    countdown_selected: AtomicU32,
    edit_field: AtomicEditField,
    edit_speed: AtomicU32,
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
            edit_speed: AtomicU32::new(SPEED_STEPS),
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
        const HOLD_DURATION_TICK: u32 = 10;
        const MAX_SPEED: f32 = 5.0;

        if j.position().is_none() {
            return;
        }

        if j.clicked() {
            let pos = j.position().as_ref().unwrap();

            use crate::joystick::JoystickButton::*;

            match pos {
                // Up pressed
                Up => self.edit_field_add(1.0),
                // Down pressed
                Down => self.edit_field_sub(1.0),
                // Left pressed
                Left => self.edit_field_prev(),
                // Right pressed
                Right => self.edit_field_next(),
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

            let speed_raw = self.edit_speed.load(Ordering::Acquire);
            let speed: f32 = speed_raw as f32 / SPEED_STEPS as f32;

            use crate::joystick::JoystickButton::*;
            match pos {
                // Up pressed
                Up => {
                    self.edit_field_add(speed);
                }
                // Down pressed
                Down => {
                    self.edit_field_sub(speed);
                }
                _ => {}
            }

            if speed < MAX_SPEED {
                self.edit_speed.store(speed_raw + 1, Ordering::Release);
            }
        } else {
            self.edit_speed.store(SPEED_STEPS, Ordering::Relaxed);
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

    /// Add rounded `edit_speed` value to current edit value
    fn edit_field_add(&self, speed: f32) {
        let to_add = match self.edit_field.load(Ordering::Relaxed) {
            EditField::Hours => 60 * 60 * speed as u32,
            EditField::Minutes => 60 * speed as u32,
            EditField::Seconds => speed as u32,
        };

        let mut counter = self.countdown_selected.load(Ordering::Acquire);
        let diff = MAX_TIMER_COUNTDOWN - counter;
        if diff > to_add {
            counter += to_add;
        } else {
            counter = MAX_TIMER_COUNTDOWN;
        }
        self.countdown_selected.store(counter, Ordering::Release);
    }

    /// Substract rounded `edit_speed` value to current edit value
    fn edit_field_sub(&self, speed: f32) {
        let to_sub = match self.edit_field.load(Ordering::Relaxed) {
            EditField::Hours => 60 * 60 * speed as u32,
            EditField::Minutes => 60 * speed as u32,
            EditField::Seconds => speed as u32,
        };

        let mut counter = self.countdown_selected.load(Ordering::Acquire);
        if counter > to_sub {
            counter -= to_sub;
        } else {
            counter = 0;
        }
        self.countdown_selected.store(counter, Ordering::Release);
    }

    /// Switch to next edit field
    fn edit_field_next(&self) {
        let new_field = match self.edit_field.load(Ordering::Acquire) {
            EditField::Hours => EditField::Minutes,
            EditField::Minutes => EditField::Seconds,
            EditField::Seconds => EditField::Hours,
        };

        self.edit_field.store(new_field, Ordering::Release);
    }

    /// Switch to previous edit field
    fn edit_field_prev(&self) {
        let new_field = match self.edit_field.load(Ordering::Acquire) {
            EditField::Hours => EditField::Seconds,
            EditField::Minutes => EditField::Hours,
            EditField::Seconds => EditField::Minutes,
        };

        self.edit_field.store(new_field, Ordering::Release);
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
            Point { x: 64, y: 32 },
            self.state().content_style,
            Alignment::Center,
        )
        .draw(target)?;

        // Draw selector
        if int_state == TimerInternalState::Edit {
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
                EditField::Seconds => Point { x: 92, y: height },
            };

            arrow.translate(pos).draw(target)?;
        }

        Ok(())
    }
}
