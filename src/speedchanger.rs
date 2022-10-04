use core::sync::atomic::{AtomicU32, Ordering};

/// Controls speed by changing how often function will be run
pub struct SpeedChanger<const RESET_DIV: u32> {
    current_max_div: AtomicU32,
    current_div: AtomicU32,
}

impl<const RESET_DIV: u32> Default for SpeedChanger<RESET_DIV> {
    fn default() -> Self {
        Self {
            current_max_div: AtomicU32::new(RESET_DIV),
            current_div: AtomicU32::new(RESET_DIV),
        }
    }
}

impl<const RESET_DIV: u32> SpeedChanger<RESET_DIV> {
    // Executes function but only 1 time in current divider
    pub fn execute<F: Fn()>(&self, function: F) {
        if self.next_div() == 0 {
            function();
        }
    }

    pub fn reset(&self) {
        self.current_max_div.store(RESET_DIV, Ordering::Relaxed);
        self.current_div.store(RESET_DIV, Ordering::Relaxed);
    }

    pub fn decrement_max_div(&self) {
        let current = self.current_max_div.load(Ordering::Acquire);
        if current > 0 {
            self.current_max_div.fetch_sub(1, Ordering::Release);
            // Also skip current step
            self.next_div();
        } else {
            self.current_max_div.store(0, Ordering::Release);
        }
    }

    fn next_div(&self) -> u32 {
        let current = self.current_div.load(Ordering::Acquire);
        if current > 0 {
            self.current_div.fetch_sub(1, Ordering::Release) - 1 // - 1 to get new value instead old
        } else {
            let max = self.current_max_div.load(Ordering::SeqCst);
            self.current_div.store(max, Ordering::Release);
            max
        }
    }
}
