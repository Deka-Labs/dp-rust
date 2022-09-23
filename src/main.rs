#![no_std]
#![no_main]

extern crate atomic_enum;
extern crate chrono;
extern crate spin;

/// HAL library for our device
extern crate stm32f4xx_hal as hal;

/// Peripheral Access Crate for our device
pub use hal::pac;

/// Mod for formatting strings
mod format;

/// I2C that can use DMA
mod i2c;

/// RTC
mod ds3231;

/// SSD1306 driver
mod ssd1306;

/// Joystick driver
mod joystick;

mod app_state;

use panic_halt as _;

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6, SPI5, SPI4, SPI3])]
mod app {

    use cortex_m::asm::wfi;

    use embedded_graphics::pixelcolor::BinaryColor;
    use hal::gpio::*;

    use hal::prelude::*;
    use hal::timer::MonoTimerUs;

    use embedded_graphics::prelude::*;
    use spin::lock_api::RwLock;

    use crate::app_state::prelude::*;
    use crate::i2c::init_i2c1;
    use crate::i2c::I2c1HandleProtected;

    use crate::ds3231::DS3231;
    use crate::i2c::I2c1Handle;
    use crate::joystick::*;

    use crate::ssd1306::SSD1306;

    #[shared]
    struct Shared {
        app_state: RwLock<AppState>,
    }

    #[local]
    struct Local {
        led: PA5<Output>,
        display: SSD1306<'static, PA8<Output<PushPull>>, I2c1Handle>,
        rtc: DS3231<I2c1Handle>,
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
    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Init clocks
        let dp = ctx.device;

        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(100.MHz()).freeze();

        // LED indicator
        let gpioa = dp.GPIOA.split();
        let led = gpioa.pa5.into_push_pull_output();

        let mono = dp.TIM5.monotonic_us(&clocks);

        // I2C bus init
        let gpiob = dp.GPIOB.split();
        let i2c: &'static mut I2c1HandleProtected = init_i2c1(
            dp.I2C1,
            (
                gpiob.pb8.into_alternate_open_drain(),
                gpiob.pb9.into_alternate_open_drain(),
            ),
            &clocks,
        );

        // Display and sensors
        let mut display = SSD1306::new(gpioa.pa8.into_push_pull_output(), i2c);
        display.init().expect("Display init failure");

        let mut rtc = DS3231::new(i2c);

        let mut timer = dp.TIM2.counter_us(&clocks);
        if let Err(e) = timer.start(1.secs()) {
            panic!("{:?}", e);
        }

        rtc.update_time().unwrap();

        // Configure buttons
        let gpioc = dp.GPIOC.split();

        let up = ButtonPullUp::new(gpioa.pa1.into_pull_up_input());
        let down = ButtonPullUp::new(gpioc.pc0.into_pull_up_input());
        let left = ButtonPullUp::new(gpiob.pb0.into_pull_up_input());
        let right = ButtonPullUp::new(gpioa.pa4.into_pull_up_input());
        let center = ButtonPullUp::new(gpioc.pc1.into_pull_up_input());

        let joy = AccessoryShieldJoystick::new(up, down, left, right, center);

        let app_state = RwLock::new(AppState::Clock(ClockState::new(&rtc)));
        app_state.write().enter(AppSharedState::new());

        // Spawn repeating tasks
        draw::spawn().unwrap();
        handle_input::spawn().unwrap();
        tick::spawn().unwrap();

        (
            Shared { app_state },
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
    #[task(priority = 1, local=[rtc], shared = [&app_state])]
    fn change_state(ctx: change_state::Context, next: bool) {
        let mut cur_state = ctx.shared.app_state.write();

        if next {
            match *cur_state {
                AppState::Clock(_) => cur_state.switch(TimerState::new()),
                AppState::Timer(_) => cur_state.switch(StopwatchState::new()),
                AppState::Stopwatch(_) => cur_state.switch(ClockState::new(&ctx.local.rtc)),
            };
        } else {
            match *cur_state {
                AppState::Clock(_) => cur_state.switch(StopwatchState::new()),
                AppState::Timer(_) => cur_state.switch(ClockState::new(&ctx.local.rtc)),
                AppState::Stopwatch(_) => cur_state.switch(TimerState::new()),
            };
        }
    }
}
