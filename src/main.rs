#![no_std]
#![no_main]

extern crate atomic_enum;
extern crate chrono;
extern crate heapless;
extern crate spin;

/// HAL library for our device
extern crate stm32f4xx_hal as hal;

/// Peripheral Access Crate for our device
pub use hal::pac;

/// I2C that can use DMA
mod i2c;

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

/// Control changing speed of digits
mod speedchanger;

mod app_state;

use panic_halt as _;

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6, SPI5, SPI4])]
mod app {

    // Standart library imports
    use core::cell::RefCell;

    // Cortex specific
    use cortex_m::asm::wfi;

    // HAL imports
    use hal::dma::StreamsTuple;
    use hal::gpio::*;
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
    use crate::i2c::I2c1Handle;
    use crate::joystick::*;
    use crate::ssd1306::SSD1306;

    // Type defs
    pub type StopwatchTimer = crate::stopwatchtimer::StopwatchTimer<crate::pac::TIM2>;
    pub type CountdownTimer = crate::countdowntimer::CountdownTimer<crate::pac::TIM4>;
    pub type I2c1HandleProtected = Mutex<RefCell<I2c1Handle>>;

    pub type UpButton = ButtonPullUp<PA1>;
    pub type DownButton = ButtonPullUp<PC0>;
    pub type LeftButton = ButtonPullUp<PB0>;
    pub type RightButton = ButtonPullUp<PA4>;
    pub type CenterButton = ButtonPullUp<PC1>;

    pub type JoystickImpl =
        AccessoryShieldJoystick<UpButton, DownButton, LeftButton, RightButton, CenterButton>;

    #[shared]
    struct Shared {
        app_state: RwLock<AppStateHolder>,

        i2c: &'static I2c1HandleProtected,
    }

    #[local]
    struct Local {
        /// indicate work of plate. Used in `tick`
        led: PA5<Output>,

        /// Used in [`draw`]
        display: SSD1306<'static, PA8<Output<PushPull>>, I2c1Handle>,

        /// Handles input
        joy: JoystickImpl,

        /// Stopwatch
        stopwatch: &'static StopwatchTimer,
        /// Countdown
        countdown: &'static CountdownTimer,
    }

    #[monotonic(binds = TIM5, default = true)]
    type MicrosecMono = MonoTimerUs<crate::pac::TIM5>;

    /// Init function running on reset
    ///
    /// * Configures clocks to 100 MHz
    /// * Configures PA5(User LED) for tick indication
    /// * Creates I2C bus, display, RTC
    /// * Configures joystick
    /// * Starts repeating tasks
    #[init(local = [
        _stopwatch: Option<StopwatchTimer> = None,
        _countdown: Option<CountdownTimer> = None,
        _i2c_bus: Option<I2c1HandleProtected> = None,
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
        let stopwatch_ref = ctx.local._stopwatch.as_ref().unwrap();

        *ctx.local._countdown = Some(CountdownTimer::new(
            dp.TIM4,
            hal::interrupt::TIM4,
            buzzer,
            &clocks,
        ));
        let countdown_ref = ctx.local._countdown.as_ref().unwrap();

        // LED indicator

        let led = gpioa.pa5.into_push_pull_output();

        // I2C bus init
        let gpiob = dp.GPIOB.split();
        let i2c = dp.I2C1.i2c(
            (
                gpiob.pb8.into_alternate_open_drain(),
                gpiob.pb9.into_alternate_open_drain(),
            ),
            400.kHz(),
            &clocks,
        );

        let streams = StreamsTuple::new(dp.DMA1);

        let i2c_dma = i2c.use_dma(streams.1, streams.0);
        *ctx.local._i2c_bus = Some(Mutex::new(RefCell::new(i2c_dma)));

        let i2c_bus_ref = ctx.local._i2c_bus.as_ref().unwrap();

        // Display and sensors
        let mut display = SSD1306::new(gpioa.pa8.into_push_pull_output(), i2c_bus_ref);
        display.init().expect("Display init failure");

        let rtc = DS3231::new(i2c_bus_ref);
        rtc.update_time().unwrap();

        // Configure buttons
        let gpioc = dp.GPIOC.split();

        let up = ButtonPullUp::new(gpioa.pa1.into_pull_up_input());
        let down = ButtonPullUp::new(gpioc.pc0.into_pull_up_input());
        let left = ButtonPullUp::new(gpiob.pb0.into_pull_up_input());
        let right = ButtonPullUp::new(gpioa.pa4.into_pull_up_input());
        let center = ButtonPullUp::new(gpioc.pc1.into_pull_up_input());

        let joy = AccessoryShieldJoystick::new(up, down, left, right, center);

        let clock_state = ClockState::new(rtc);
        let stopwatch_state = StopwatchState::new(stopwatch_ref);
        let timer_state = TimerState::new(countdown_ref);

        let app_state = RwLock::new(AppStateHolder::new(
            clock_state,
            timer_state,
            stopwatch_state,
            AppSharedState::default(),
        ));

        // Spawn repeating tasks
        draw::spawn().unwrap();
        handle_input::spawn().unwrap();
        tick::spawn().unwrap();

        (
            Shared {
                app_state,
                i2c: i2c_bus_ref,
            },
            Local {
                led,
                display,
                joy,
                stopwatch: stopwatch_ref,
                countdown: countdown_ref,
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
    #[task(local = [display], shared = [&app_state], priority = 1, capacity = 1)]
    fn draw(ctx: draw::Context) {
        draw::spawn_after(100.millis()).ok();

        let display = ctx.local.display;

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
    #[task(priority = 1, local=[], shared = [&app_state])]
    fn change_state(ctx: change_state::Context, next: bool) {
        let mut cur_state = ctx.shared.app_state.write();

        if next {
            cur_state.next();
        } else {
            cur_state.prev();
        }
    }

    /// Handles stopwacth interrupts
    #[task(binds = TIM2, local = [stopwatch], priority = 5)]
    fn tim_stopwatch_it(ctx: tim_stopwatch_it::Context) {
        ctx.local.stopwatch.increment();
    }

    /// Handles stopwacth interrupts
    #[task(binds = TIM4, local = [countdown], priority = 5)]
    fn tim_countdown_it(ctx: tim_countdown_it::Context) {
        ctx.local.countdown.handle_it();
    }

    #[task(binds = DMA1_STREAM1, shared = [&i2c], priority = 7)]
    fn i2c_dma_it(ctx: i2c_dma_it::Context) {
        critical_section::with(|cs| {
            let mut c = ctx.shared.i2c.borrow(cs).borrow_mut();
            c.handle_dma_interrupt();
        })
    }

    #[task(binds = I2C1_ER, shared = [&i2c], priority = 7)]
    fn i2c_er_it(ctx: i2c_er_it::Context) {
        critical_section::with(|cs| {
            let mut c = ctx.shared.i2c.borrow(cs).borrow_mut();
            c.handle_error_interrupt();
        })
    }
}
