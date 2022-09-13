#![no_std]
#![no_main]

/// Mod for formatting strings
mod format;

/// I2C that can use DMA
mod i2c;

/// Temperature sensor
mod lm75b;

/// RTC
mod ds3231;

/// SSD1306 driver
mod ssd1306;

use chrono::prelude::*;
/// Peripheral Access Crate for our device
pub use stm32f4xx_hal::pac;

/// HAL library for our device
pub use stm32f4xx_hal as hal;

use panic_halt as _;

#[derive(Clone, Default)]
pub struct DisplayInfo {
    datetime: DateTime<Utc>,
    temperature: f32,
}

unsafe impl Sync for DisplayInfo {}

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6])]
mod app {

    use chrono::Duration;

    use embedded_graphics::mono_font::ascii::FONT_9X15;
    use embedded_graphics::mono_font::MonoTextStyleBuilder;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::*;
    use embedded_graphics::text::Text;

    use crate::ds3231::DS3231;
    use crate::format::format_time;
    use crate::hal::gpio::{OpenDrain, Output, PushPull, AF4, PA5, PA8, PB8, PB9};
    use crate::hal::prelude::*;
    use crate::hal::timer::MonoTimerUs;
    use crate::DisplayInfo;

    use crate::pac::I2C1;

    use crate::format::format_string;
    use crate::i2c::I2c;
    use crate::lm75b::LM75B;
    use crate::ssd1306::SSD1306;

    #[shared]
    struct Shared {
        bus: I2c<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>,
        display_info: DisplayInfo,
    }

    #[local]
    struct Local {
        led: PA5<Output>,
        display: SSD1306<PA8<Output<PushPull>>>,
        temp_probe: LM75B,
        rtc: DS3231,
    }

    #[monotonic(binds = TIM5, default = true)]
    type MicrosecMono = MonoTimerUs<crate::pac::TIM5>;

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
        let mut i2c = I2c::new(
            dp.I2C1,
            (
                gpiob.pb8.into_alternate_open_drain(),
                gpiob.pb9.into_alternate_open_drain(),
            ),
            400.kHz(),
            &clocks,
        );

        // Display and sensors
        let mut display = SSD1306::new(gpioa.pa8.into_push_pull_output());
        display.init(&mut i2c).expect("Display init failure");

        let temp_probe = LM75B::new([false; 3]);
        let mut rtc = DS3231::new();

        rtc.update_time(&mut i2c).unwrap();
        let di = DisplayInfo {
            temperature: 0.0_f32, // Will update in grab_temperature task
            datetime: rtc.time().clone(),
        };

        // Spawn repeating tasks
        tick::spawn().unwrap();
        grab_temperature::spawn().unwrap();

        (
            Shared {
                bus: i2c,
                display_info: di,
            },
            Local {
                led,
                display,
                temp_probe,
                rtc,
            },
            init::Monotonics(mono),
        )
    }

    #[idle(local = [ ], shared = [bus])]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            draw::spawn().ok();
        }
    }

    #[task(local = [led], shared=[display_info])]
    fn tick(ctx: tick::Context) {
        tick::spawn_after(1000.millis()).unwrap();
        ctx.local.led.toggle();

        // Add 1 seconds without request time from RTC
        let mut di = ctx.shared.display_info;
        di.lock(|i| i.datetime = i.datetime + Duration::seconds(1))
    }

    #[task(local=[temp_probe], shared=[bus, display_info])]
    fn grab_temperature(mut ctx: grab_temperature::Context) {
        // LM75B updates temperature reading each 100 ms
        grab_temperature::spawn_after(100.millis()).unwrap();

        let lm75b = &mut *ctx.local.temp_probe;
        let temp = ctx.shared.bus.lock(|bus| lm75b.temperature(bus).unwrap());

        let mut di = ctx.shared.display_info;
        di.lock(|i| i.temperature = temp);
    }

    #[task(local = [display], shared = [bus, display_info])]
    fn draw(mut ctx: draw::Context) {
        // Styles
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_9X15)
            .text_color(BinaryColor::On)
            .build();

        // Clear display content
        let display = &mut *ctx.local.display;
        display.clear();

        // Buffer for render strings
        let mut str_buf = [0_u8; 64];
        // Shared display info
        let mut di = ctx.shared.display_info;

        // Draw time
        let time_str =
            di.lock(|display_info| format_time(&mut str_buf, &display_info.datetime).unwrap());

        let text = Text::with_alignment(
            time_str,
            Point { x: 64, y: 12 },
            text_style.clone(),
            embedded_graphics::text::Alignment::Center,
        );
        text.draw(display).unwrap();

        // Draw temperature
        str_buf.fill(0);
        let temp_str = di.lock(|display_info| {
            format_string(
                &mut str_buf,
                format_args!("Temp: {:.1}", display_info.temperature),
            )
            .unwrap()
        });
        let text = Text::with_alignment(
            temp_str,
            Point { x: 64, y: 32 },
            text_style.clone(),
            embedded_graphics::text::Alignment::Center,
        );
        text.draw(display).unwrap();

        // Swap buffers to display
        ctx.shared.bus.lock(|bus| {
            display.swap(bus);
        })
    }
}
