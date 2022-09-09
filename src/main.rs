#![no_std]
#![no_main]

use panic_halt as _;

pub use stm32f4xx_hal::pac;

#[rtic::app(device = crate::pac, peripherals = true)]
mod app {

    use stm32f4xx_hal::{
        gpio::{Output, PushPull, PA5},
        prelude::*,
        timer::DelayUs,
    };

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        led: PA5<Output<PushPull>>,
        delay: DelayUs<crate::pac::TIM5>,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let device = ctx.device;

        let rcc = device.RCC.constrain();
        let clocks = rcc.cfgr.use_hse(8.MHz()).sysclk(100.MHz()).freeze();

        let delay = device.TIM5.delay_us(&clocks);

        let gpioa = device.GPIOA.split();
        let led = gpioa.pa5.into_push_pull_output();

        (Shared {}, Local { led, delay }, init::Monotonics())
    }

    #[idle(local = [led, delay])]
    fn idle(ctx: idle::Context) -> ! {
        loop {
            ctx.local.led.set_high();
            ctx.local.delay.delay_ms(1000_u32);
            ctx.local.led.set_low();
            ctx.local.delay.delay_ms(1000_u32);

            ctx.local.led.set_high();
            ctx.local.delay.delay_ms(100_u32);
            ctx.local.led.set_low();
            ctx.local.delay.delay_ms(100_u32);
        }
    }
}
