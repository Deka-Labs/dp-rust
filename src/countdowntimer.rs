use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cortex_m::peripheral::NVIC;
use hal::pac::Interrupt;
use hal::prelude::*;
use hal::rcc::Clocks;
use hal::timer::Event;
use hal::timer::{Counter, Instance};

use crate::buzzer::Buzzer;

const TIMER_TARGET_FREQ: u32 = 2000;
const TIMER_MS_STEP: u32 = 1000;

pub struct CountdownTimer<TIM: Instance> {
    timer: RefCell<Counter<TIM, TIMER_TARGET_FREQ>>,
    buzzer: Buzzer,
    it: Interrupt,

    countdown: AtomicU32,
    started: AtomicBool,
}

impl<TIM: Instance> CountdownTimer<TIM> {
    pub fn new(timer: TIM, tim_interrupt: Interrupt, buzzer: Buzzer, clocks: &Clocks) -> Self {
        let mut tim = timer.counter(clocks);
        tim.start(TIMER_MS_STEP.millis())
            .expect("Failed to start timer");
        tim.listen(Event::Update);
        NVIC::mask(tim_interrupt);

        Self {
            timer: RefCell::new(tim),
            buzzer,
            it: tim_interrupt,

            countdown: AtomicU32::new(0),
            started: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn start(&self, countdown_seconds: u32) {
        self.countdown.store(countdown_seconds, Ordering::Relaxed);
        self.started.store(true, Ordering::Relaxed);

        // Restart timer
        self.timer
            .borrow_mut()
            .start(TIMER_MS_STEP.millis())
            .unwrap();

        // Safe: TIM interrupts doesn't affect any critical-section locked resources
        unsafe {
            NVIC::unmask(self.it);
        }
    }

    #[inline]
    pub fn stop(&self) {
        self.countdown.store(0, Ordering::Relaxed);
        self.started.store(false, Ordering::Relaxed);

        self.buzzer.disable();

        NVIC::mask(self.it);
    }

    #[inline]
    pub fn handle_it(&self) {
        self.timer.borrow_mut().clear_interrupt(Event::Update);
        if self.started() {
            let c = self.countdown.load(Ordering::Acquire);
            if c > 0 {
                self.countdown.fetch_sub(1, Ordering::Release);
            } else {
                self.buzzer.enable();
            }
        }
    }

    #[inline]
    pub fn countdown(&self) -> u32 {
        self.countdown.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn started(&self) -> bool {
        self.started.load(Ordering::Relaxed)
    }
}

unsafe impl<TIM: Instance> Sync for CountdownTimer<TIM> {}
unsafe impl<TIM: Instance> Send for CountdownTimer<TIM> {}
