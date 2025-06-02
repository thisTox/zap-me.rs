#![no_std]
#![no_main]

use core::cell::RefCell;
use cortex_m_rt::entry;
use embedded_hal::digital::v2::OutputPin;
use panic_halt as _;
use rp2040_hal::{Clock, gpio::Pins, pac, sio::Sio, timer::Timer, watchdog::Watchdog};

#[unsafe(link_section = ".boot2")]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

const XTAL_FREQ_HZ: u32 = 12_000_000u32;

use zap_me::ch8803::Transmitter as ZapMe;

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    let clocks = rp2040_hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let timer = RefCell::new(Timer::new(pac.TIMER, &mut pac.RESETS, &clocks));

    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let Pins {
        gpio16: zap,
        gpio25: led,
        ..
    } = pins;

    let zap = zap.into_push_pull_output();
    let mut led = led.into_push_pull_output();

    let mut collar = ZapMe::builder()
        .pin(zap)
        .delay(&timer)
        .now_fn(|| timer.borrow_mut().get_counter())
        .id(0x0D25)
        .build();

    loop {
        led.set_high().unwrap();
        collar.channel(0).beep_ms(500);
        delay.delay_ms(1000);
        led.set_low().unwrap();
        delay.delay_ms(2000);
    }
}
