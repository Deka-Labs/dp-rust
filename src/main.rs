#![no_std]
#![no_main]

/// Mod for formatting strings
mod format;

/// I2C that can use DMA
mod i2c;

/// Temperature sensor
mod lm75b;

/// SSD1306 driver
mod ssd1306;

/// Peripheral Access Crate for our device
pub use stm32f4xx_hal::pac;

/// HAL library for our device
pub use stm32f4xx_hal as hal;

use panic_halt as _;

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6])]
mod app {

    use embedded_graphics::mono_font::ascii::FONT_10X20;
    use embedded_graphics::mono_font::MonoTextStyle;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::*;
    use embedded_graphics::text::Text;

    use crate::hal::gpio::{OpenDrain, Output, PushPull, AF4, PA5, PA8, PB8, PB9};
    use crate::hal::prelude::*;
    use crate::hal::timer::MonoTimerUs;

    use crate::pac::I2C1;

    use crate::format::format_string;
    use crate::i2c::I2c;
    use crate::lm75b::LM75B;
    use crate::ssd1306::SSD1306;

    #[shared]
    struct Shared {
        bus: I2c<I2C1, (PB8<AF4<OpenDrain>>, PB9<AF4<OpenDrain>>)>,
    }

    #[local]
    struct Local {
        led: PA5<Output>,
        display: SSD1306<PA8<Output<PushPull>>>,
        temp: LM75B,
    }

    #[monotonic(binds = TIM5, default = true)]
    type MicrosecMono = MonoTimerUs<crate::pac::TIM5>;

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let dp = ctx.device;

        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(100.MHz()).freeze();

        let gpioa = dp.GPIOA.split();
        let led = gpioa.pa5.into_push_pull_output();

        let mono = dp.TIM5.monotonic_us(&clocks);

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

        let mut display = SSD1306::new(gpioa.pa8.into_push_pull_output());
        display.init(&mut i2c).expect("Display init failure");

        let temp = LM75B::new([false; 3]);

        tick::spawn().unwrap();

        (
            Shared { bus: i2c },
            Local { led, display, temp },
            init::Monotonics(mono),
        )
    }

    #[idle(local = [display, temp], shared = [bus])]
    fn idle(mut ctx: idle::Context) -> ! {
        loop {
            let lm75b = &mut *ctx.local.temp;
            let temp = ctx.shared.bus.lock(|bus| lm75b.temperature(bus).unwrap());

            let display = &mut *ctx.local.display;

            display.clear();
            let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);

            let mut str_buf = [0_u8; 64];
            let str = format_string(&mut str_buf, format_args!("Temp: {:.1}", temp)).unwrap();

            let text = Text::new(str, Point { x: 0, y: 30 }, text_style);

            text.draw(display).unwrap();
            ctx.shared.bus.lock(|bus| {
                display.swap(bus);
            })
        }
    }

    #[task(local = [led])]
    fn tick(ctx: tick::Context) {
        tick::spawn_after(200.millis()).unwrap();
        ctx.local.led.toggle();
    }
}
