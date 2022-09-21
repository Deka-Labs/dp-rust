#![no_std]
#![no_main]

extern crate chrono;
/// HAL library for our device
extern crate stm32f4xx_hal as hal;

/// Peripheral Access Crate for our device
pub use hal::pac;

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

/// Joystick driver
mod joystick;

/// All information on display
mod displayinfo;

use panic_halt as _;

#[rtic::app(device = crate::pac, peripherals = true, dispatchers = [USART6, SPI5, SPI4])]
mod app {

    use cortex_m::asm::wfi;
    use hal::gpio::*;
    use hal::i2c::I2c;
    use hal::prelude::*;
    use hal::timer::MonoTimerUs;

    use crate::pac::I2C1;

    use embedded_graphics::mono_font::ascii::FONT_10X20;
    use embedded_graphics::mono_font::MonoTextStyleBuilder;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::Circle;
    use embedded_graphics::primitives::PrimitiveStyleBuilder;
    use embedded_graphics::primitives::Triangle;
    use embedded_graphics::text::Text;

    use crate::displayinfo::DateTime;
    use crate::displayinfo::DisplayInfo;
    use crate::displayinfo::RTCField;
    use crate::ds3231::DS3231;
    use crate::format::format_string;
    use crate::format::format_time;
    use crate::joystick::*;
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
        let di = DisplayInfo::from_datetime(rtc.time());

        // Configure buttons
        let gpioc = dp.GPIOC.split();

        let up = ButtonPullUp::new(gpioa.pa1.into_pull_up_input());
        let down = ButtonPullUp::new(gpioc.pc0.into_pull_up_input());
        let left = ButtonPullUp::new(gpiob.pb0.into_pull_up_input());
        let right = ButtonPullUp::new(gpioa.pa4.into_pull_up_input());
        let center = ButtonPullUp::new(gpioc.pc1.into_pull_up_input());

        let joy = AccessoryShieldJoystick::new(up, down, left, right, center);

        // Spawn repeating tasks
        tick::spawn().unwrap();
        grab_temperature::spawn().unwrap();
        handle_input::spawn().unwrap();
        draw::spawn().unwrap();

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
    #[task(local = [led], shared=[display_info], priority = 5)]
    fn tick(ctx: tick::Context) {
        tick::spawn_after(1000.millis()).unwrap();
        ctx.local.led.toggle();

        // Add 1 seconds without request time from RTC
        let mut di = ctx.shared.display_info;
        di.lock(|i| i.tick())
    }

    /// Send new time to RTC
    #[task(local = [rtc], shared = [bus], priority = 3)]
    fn send_time_to_rtc(mut ctx: send_time_to_rtc::Context, time: DateTime) {
        let rtc = ctx.local.rtc;
        ctx.shared.bus.lock(|bus| {
            rtc.set_time(bus, time).expect("Failed to send time");
        });
    }

    /// The task gets temperature reading from thermometer
    #[task(local=[temp_probe], shared=[bus, display_info], priority = 3)]
    fn grab_temperature(mut ctx: grab_temperature::Context) {
        // LM75B updates temperature reading each 100 ms
        grab_temperature::spawn_after(100.millis()).unwrap();

        let lm75b = &mut *ctx.local.temp_probe;
        let temp = ctx.shared.bus.lock(|bus| lm75b.temperature(bus).unwrap());

        let mut di = ctx.shared.display_info;
        di.lock(|i| i.set_temperature(temp));
    }

    /// handle_input handles joystick
    #[task(local = [joy, speed: f32 = 1.0, sync_required: bool = false], shared = [display_info])]
    fn handle_input(mut ctx: handle_input::Context) {
        const MAX_SPEED: f32 = 5.0;
        let update_interval = 100.millis();
        handle_input::spawn_after(update_interval).unwrap();

        let speed = ctx.local.speed;
        let sync_required = ctx.local.sync_required;

        let di = &mut ctx.shared.display_info;

        let j = ctx.local.joy;

        j.update();

        let state = j.position();

        if let Some(pos) = state {
            di.lock(|i| {
                use JoystickButton::*;

                // On click
                if j.clicked() {
                    match &pos {
                        // Up pressed
                        Up => {
                            i.add_time(1);
                            *sync_required = true;
                        }
                        // Down pressed
                        Down => {
                            i.sub_time(1);
                            *sync_required = true;
                        }
                        // Left pressed
                        Left => {
                            i.next_field();
                        }
                        // Right pressed
                        Right => {
                            i.prev_field();
                        }
                        _ => {}
                    }
                }

                // On hold action
                if j.hold_time() * update_interval > 500.millis::<1, 1_000_000>() {
                    match &pos {
                        // Up pressed
                        Up => {
                            i.add_time(*speed as i64);
                        }
                        // Down pressed
                        Down => {
                            i.sub_time(*speed as i64);
                        }
                        _ => {}
                    }

                    if *speed < MAX_SPEED {
                        *speed += 0.25
                    }
                } else {
                    *speed = 1.0;
                }
            });
        } else {
            // else - Joystick unpressed

            // Save changes
            if j.just_unpressed() && *sync_required {
                *sync_required = false;
                let time = di.lock(|s| s.reset_seconds().clone());
                send_time_to_rtc::spawn(time).unwrap();
            }
        }
    }

    /// Draw task draws content of `display_info` onto screen
    #[task(local = [display], shared = [bus, display_info], priority = 1, capacity = 1)]
    fn draw(mut ctx: draw::Context) {
        draw::spawn_after(100.millis()).unwrap();

        // Styles
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(BinaryColor::On)
            .build();

        let line_style = PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(BinaryColor::On)
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
            di.lock(|display_info| format_time(&mut str_buf, display_info.datetime()).unwrap());

        let text = Text::with_alignment(
            time_str,
            Point { x: 64, y: 32 },
            text_style.clone(),
            embedded_graphics::text::Alignment::Center,
        );
        text.draw(display).unwrap();
        // Draw selected to edit line
        {
            let y = 14;
            let height = 10;
            let width = 6;
            let pos_hours = 33;
            let pos_min = 64;

            let p1 = match di.lock(|i| i.field()) {
                RTCField::Hours => Point::new(pos_hours, y),
                RTCField::Minutes => Point::new(pos_min, y),
            };

            Triangle::new(
                p1,
                p1 + Point::new(-width / 2, -height),
                p1 + Point::new(width / 2, -height),
            )
            .into_styled(line_style)
            .draw(display)
            .unwrap();
        }

        // Draw temperature
        {
            str_buf.fill(0);
            let temp_str = di.lock(|display_info| {
                format_string(
                    &mut str_buf,
                    format_args!("{:.1} C", display_info.temperature()),
                )
                .unwrap()
            });
            let text = Text::with_alignment(
                temp_str,
                Point { x: 64, y: 60 },
                text_style.clone(),
                embedded_graphics::text::Alignment::Center,
            );
            text.draw(display).unwrap();

            Circle::with_center(Point::new(82, 47), 4)
                .into_styled(line_style)
                .draw(display)
                .unwrap();
        }

        // Swap buffers to display
        ctx.shared.bus.lock(|bus| {
            display.swap(bus);
        })
    }
}
