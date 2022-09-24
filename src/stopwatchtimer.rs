use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use cortex_m::peripheral::NVIC;
use stm32f4xx_hal::{
    pac::Interrupt,
    prelude::*,
    rcc::Clocks,
    timer::{CounterUs, Event, Instance, TimerExt},
};

/// Step between timer interrupts
const TIMER_MS_STEP: u32 = 100;

pub struct StopwatchTimer<TIM: Instance> {
    timer: RefCell<CounterUs<TIM>>,
    it: Interrupt,
    elapsed: AtomicU32,

    started: AtomicBool,
}

impl<TIM: Instance> StopwatchTimer<TIM> {
    pub fn new(timer: TIM, tim_interrupt: Interrupt, clocks: &Clocks) -> Self {
        let mut tim = timer.counter(clocks);
        tim.start(TIMER_MS_STEP.millis())
            .expect("Failed to start timer");
        tim.listen(Event::Update);
        NVIC::mask(tim_interrupt);

        Self {
            timer: RefCell::new(tim),
            it: tim_interrupt,
            elapsed: AtomicU32::new(0),
            started: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn start(&self) {
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
        self.elapsed.store(0, Ordering::Relaxed);
        self.started.store(false, Ordering::Relaxed);
        NVIC::mask(self.it);
    }

    #[inline]
    pub fn pause(&self) {
        self.started.store(false, Ordering::Relaxed);
        NVIC::mask(self.it);
    }

    #[inline]
    pub fn increment(&self) {
        self.timer.borrow_mut().clear_interrupt(Event::Update);
        if self.started() {
            self.elapsed.fetch_add(TIMER_MS_STEP, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn elapsed(&self) -> u32 {
        self.elapsed.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn started(&self) -> bool {
        self.started.load(Ordering::Relaxed)
    }
}

unsafe impl<TIM: Instance> Sync for StopwatchTimer<TIM> {}
unsafe impl<TIM: Instance> Send for StopwatchTimer<TIM> {}
