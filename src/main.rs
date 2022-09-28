#![no_std]
#![no_main]

extern crate atomic_enum;
extern crate chrono;
extern crate cortex_m_rt;
extern crate heapless;
extern crate spin;

/// HAL library for our device
extern crate stm32f4xx_hal as hal;

/// Peripheral Access Crate for our device
pub use hal::pac;

/// Standart
mod i2c;

/// Async interrupt based I2C
mod i2c_async;

/// RTC
mod ds3231;

/// SSD1306 driver
mod ssd1306;

/// Joystick driver
mod joystick;

/// Stopwatch abstraction for Timer
mod stopwatchtimer;

/// Countdown timer to implement timer
mod countdowntimer;

/// Buzzer to make sounds
mod buzzer;

mod app_state;

use panic_halt as _;

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6, SPI5, SPI4, SPI3])]
mod app {

    // Standart library imports
    use core::cell::RefCell;

    // Cortex specific
    use cortex_m::asm::wfi;

    // HAL imports
    use hal::gpio::*;
    use hal::pac::I2C1;
    use hal::prelude::*;
    use hal::timer::MonoTimerUs;

    // External helpers libraries
    use critical_section::Mutex;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::*;
    use spin::lock_api::RwLock;

    // This crate exports
    use crate::app_state::prelude::*;
    use crate::buzzer::Buzzer;
    use crate::ds3231::DS3231;
    use crate::i2c_async::*;
    use crate::joystick::*;
    use crate::ssd1306::SSD1306;

    // Type defs
    pub type StopwatchTimer = crate::stopwatchtimer::StopwatchTimer<crate::pac::TIM2>;
    pub type CountdownTimer = crate::countdowntimer::CountdownTimer<crate::pac::TIM4>;

    pub type I2c1Handle = I2c<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>;
    pub type I2c1HandleProtected = Mutex<RefCell<I2c1Handle>>;

    #[shared]
    struct Shared {
        app_state: RwLock<AppState>,
        stopwatch: &'static StopwatchTimer,
        countdown: &'static CountdownTimer,
    }

    #[local]
    struct Local {
        /// indicate work of plate. Used in `tick`
        led: PA5<Output>,

        /// Used in `draw`
        display: SSD1306<'static, PA8<Output<PushPull>>, I2c1Handle>,

        /// Used to passthrough in ClockState in `change_state`
        rtc: DS3231<I2c1Handle>,

        /// Handles input
        joy: AccessoryShieldJoystick<
            ButtonPullUp<Pin<'A', 1>>,
            ButtonPullUp<Pin<'C', 0>>,
            ButtonPullUp<Pin<'B', 0>>,
            ButtonPullUp<Pin<'A', 4>>,
            ButtonPullUp<Pin<'C', 1>>,
        >,
    }

    #[monotonic(binds = TIM5, default = true)]
    type MicrosecMono = MonoTimerUs<crate::pac::TIM5>;

    /// Init function running on reset
    ///
    /// * Configures clocks to 100 MHz
    /// * Configures PA5(User LED) for tick indication
    /// * Creates I2C bus, display, temperature sensor, RTC
    /// * Configures joystick
    /// * Starts repeating tasks
    #[init(local = [
        _stopwatch: Option<StopwatchTimer> = None,
        _countdown: Option<CountdownTimer> = None,
        _i2c_bus: Option<I2c1Handle> = None,
    ])]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Init clocks
        let dp = ctx.device;

        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(100.MHz()).freeze();

        // Timers
        let mono = dp.TIM5.monotonic_us(&clocks);

        let gpioa = dp.GPIOA.split();
        let buzzer = Buzzer::new(dp.TIM3, gpioa.pa7, &clocks);

        *ctx.local._stopwatch = Some(StopwatchTimer::new(dp.TIM2, hal::interrupt::TIM2, &clocks));
        *ctx.local._countdown = Some(CountdownTimer::new(
            dp.TIM4,
            hal::interrupt::TIM4,
            buzzer,
            &clocks,
        ));

        // LED indicator

        let led = gpioa.pa5.into_push_pull_output();

        // I2C bus init
        let gpiob = dp.GPIOB.split();
        let i2c = I2c::new(
            dp.I2C1,
            (
                gpiob.pb8.into_alternate_open_drain(),
                gpiob.pb9.into_alternate_open_drain(),
            ),
            400.kHz(),
            &clocks,
        );

        *ctx.local._i2c_bus = Some(i2c);
        let i2c_bus_ref = ctx.local._i2c_bus.as_ref().unwrap();

        // Display and sensors
        let display = SSD1306::new(gpioa.pa8.into_push_pull_output(), i2c_bus_ref);

        let rtc = DS3231::new(i2c_bus_ref);

        // Configure buttons
        let gpioc = dp.GPIOC.split();

        let up = ButtonPullUp::new(gpioa.pa1.into_pull_up_input());
        let down = ButtonPullUp::new(gpioc.pc0.into_pull_up_input());
        let left = ButtonPullUp::new(gpiob.pb0.into_pull_up_input());
        let right = ButtonPullUp::new(gpioa.pa4.into_pull_up_input());
        let center = ButtonPullUp::new(gpioc.pc1.into_pull_up_input());

        let joy = AccessoryShieldJoystick::new(up, down, left, right, center);

        let app_state = RwLock::new(AppState::Timer(TimerState::new(
            ctx.local._countdown.as_ref().unwrap(),
        )));
        app_state.write().enter(AppSharedState::new());

        // Spawn repeating tasks
        draw::spawn().unwrap();
        handle_input::spawn().unwrap();
        tick::spawn().unwrap();

        (
            Shared {
                app_state,
                stopwatch: ctx.local._stopwatch.as_ref().unwrap(),
                countdown: ctx.local._countdown.as_ref().unwrap(),
            },
            Local {
                led,
                display,
                rtc,
                joy,
            },
            init::Monotonics(mono),
        )
    }

    /// Idle function runs when nothing to do
    /// Used for sleep
    #[idle(local = [], shared = [])]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            wfi();
        }
    }

    /// tick is top-priority task. It updates clock without sync with real RTC module
    #[task(local = [led], shared=[&app_state], priority = 5)]
    fn tick(ctx: tick::Context) {
        tick::spawn_after(1000.millis()).unwrap();
        ctx.local.led.toggle();

        if let Some(s) = ctx.shared.app_state.try_read() {
            s.tick();
        }
    }

    /// handle_input handles joystick
    #[task(local = [joy], shared = [&app_state], priority = 3)]
    fn handle_input(ctx: handle_input::Context) {
        let update_interval = 50.millis();
        handle_input::spawn_after(update_interval).unwrap();

        let j = ctx.local.joy;
        j.update();

        if let Some(s) = ctx.shared.app_state.try_read() {
            s.handle_input(j);
        }
    }

    /// Draw task draws content of `display_info` onto screen
    #[task(local = [display], shared = [&app_state], priority = 3, capacity = 1)]
    fn draw(ctx: draw::Context) {
        draw::spawn_after(200.millis()).ok();

        let display = ctx.local.display;
        if !display.initialized() {
            display.init().unwrap();
        }

        // We will skip usage if borrowed mutably beacuse it is means that we're changing state
        if let Some(s) = ctx.shared.app_state.try_read() {
            display.clear(BinaryColor::Off).unwrap();

            s.draw(display).ok();

            // Swap buffers to display
            display.swap();

            let _s = 0;
        }
    }

    /// Task for switch next state
    /// Should be lowest priority
    #[task(priority = 1, local=[rtc], shared = [&app_state, &stopwatch, &countdown])]
    fn change_state(ctx: change_state::Context, next: bool) {
        let mut cur_state = ctx.shared.app_state.write();

        let stopwatch = ctx.shared.stopwatch;
        let countdown = ctx.shared.countdown;

        if next {
            match *cur_state {
                AppState::Clock(_) => cur_state.switch(TimerState::new(countdown)),
                AppState::Timer(_) => cur_state.switch(StopwatchState::new(stopwatch)),
                AppState::Stopwatch(_) => cur_state.switch(ClockState::new(&ctx.local.rtc)),
            };
        } else {
            match *cur_state {
                AppState::Clock(_) => cur_state.switch(StopwatchState::new(stopwatch)),
                AppState::Timer(_) => cur_state.switch(ClockState::new(&ctx.local.rtc)),
                AppState::Stopwatch(_) => cur_state.switch(TimerState::new(countdown)),
            };
        }
    }

    /// Handles stopwacth interrupts
    #[task(binds = TIM2, shared = [&stopwatch], priority = 5)]
    fn tim_stopwatch_it(ctx: tim_stopwatch_it::Context) {
        ctx.shared.stopwatch.increment();
    }

    /// Handles stopwacth interrupts
    #[task(binds = TIM4, shared = [&countdown], priority = 5)]
    fn tim_countdown_it(ctx: tim_countdown_it::Context) {
        ctx.shared.countdown.handle_it();
    }

    #[task(binds = I2C1_EV, shared=[], priority = 7)]
    fn i2c1_ev(_ctx: i2c1_ev::Context) {
        unsafe { I2c::<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>::handle_event_interrupt() }
    }

    #[task(binds = I2C1_ER, shared=[], priority = 7)]
    fn i2c1_er(_ctx: i2c1_er::Context) {
        unsafe { I2c::<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>::handle_error_interrupt() }
    }
}
