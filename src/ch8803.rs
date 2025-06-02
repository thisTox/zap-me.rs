use core::cell::RefCell;
use embedded_hal::blocking::delay::DelayUs;
use embedded_hal::digital::v2::OutputPin;
use typed_builder::TypedBuilder;

const PULSE_LEN: u16 = 1016;
const ZERO_LEN: u16 = 292;
const ONE_LEN: u16 = 804;

pub type Instant = fugit::Instant<u64, 1, 1_000_000>;
pub type Duration = fugit::Duration<u32, 1, 1_000_000>;

pub trait InstantFn: Fn() -> Instant {}
impl<F: Fn() -> Instant> InstantFn for F {}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
}

impl From<u8> for Channel {
    fn from(value: u8) -> Self {
        match value {
            0 => Channel::Channel1,
            1 => Channel::Channel2,
            2 => Channel::Channel3,
            _ => panic!("Invalid channel value"),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Shock = 1,
    Vibrate = 2,
    Beep = 3,
}

pub struct ChannelTransmitter<'a, 'b, PIN, DELAY, NOW>
where
    PIN: OutputPin,
    DELAY: DelayUs<u16>,
    NOW: InstantFn,
{
    device: &'b mut Transmitter<'a, PIN, DELAY, NOW>,
    channel: Channel,
}

impl<'a, 'b, PIN, DELAY, NOW> ChannelTransmitter<'a, 'b, PIN, DELAY, NOW>
where
    PIN: OutputPin,
    DELAY: DelayUs<u16>,
    NOW: InstantFn,
{
    /// Sends a shock command to the receiver.
    pub fn shock(&mut self, strength: u8, duration: impl Into<Duration>) {
        self.device.send_command(
            self.channel,
            Command::Shock,
            strength,
            duration,
        );
    }

    /// Wrapper for the [shock](Self::shock) method that takes duration in milliseconds.
    pub fn shock_ms(&mut self, strength: u8, duration: u32) {
        self.shock(strength, Duration::millis(duration));
    }

    /// Sends a vibration command to the receiver.
    pub fn vibrate(&mut self, strength: u8, duration: impl Into<Duration>) {
        self.device.send_command(
            self.channel,
            Command::Vibrate,
            strength,
            duration,
        );
    }

    /// Wrapper for the [vibrate](Self::vibrate) method that takes duration in milliseconds.
    pub fn vibrate_ms(&mut self, strength: u8, duration: u32) {
        self.vibrate(strength, Duration::millis(duration));
    }

    /// Sends a beep command to the receiver.
    pub fn beep(&mut self, duration: impl Into<Duration>) {
        self.device
            .send_command(self.channel, Command::Beep, 0, duration);
    }

    /// Wrapper for the [beep](Self::beep) method that takes duration in milliseconds.
    pub fn beep_ms(&mut self, duration: u32) {
        self.beep(Duration::millis(duration));
    }
}

#[derive(TypedBuilder)]
pub struct Transmitter<'a, PIN, DELAY, NOW>
where
    PIN: OutputPin,
    DELAY: DelayUs<u16>,
    NOW: InstantFn,
{
    /// The pin used to transmit the signal. This pin should be connected to the DATA pin
    /// of a 433 MHz transmitter module.
    pin: PIN,

    /// The delay implementation used to control the timing of the signal.
    delay: &'a RefCell<DELAY>,

    /// A function that returns the current ticks.
    now_fn: NOW,

    /// The ID of the device. This should be a unique identifier for the transmitter
    /// and is used together with the channel to pair a receiver.
    id: u16,
}

impl<'a, PIN, DELAY, NOW> Transmitter<'a, PIN, DELAY, NOW>
where
    PIN: OutputPin,
    DELAY: DelayUs<u16>,
    NOW: InstantFn,
{
    /// Binds the Transmitter to a specific channel, allowing to send actual commands.
    pub fn channel<'b>(
        &'b mut self,
        channel: impl Into<Channel>,
    ) -> ChannelTransmitter<'a, 'b, PIN, DELAY, NOW> {
        ChannelTransmitter {
            device: self,
            channel: channel.into(),
        }
    }

    fn send_command(
        &mut self,
        channel: Channel,
        command: Command,
        strength: u8,
        duration: impl Into<Duration>,
    ) {
        let mut timings: [u16; 128] = [0; 128];
        let checksum = ((self.id >> 8) as u8)
            .wrapping_add(self.id as u8)
            .wrapping_add(channel as u8)
            .wrapping_add(command as u8)
            .wrapping_add(strength);

        let mut idx = 0;

        timings[idx] = 840;
        idx += 1;
        timings[idx] = 1440;
        idx += 1;
        timings[idx] = PULSE_LEN - ZERO_LEN;
        idx += 1;

        Self::trbits(self.id as u32, 16, &mut timings, &mut idx);
        Self::trbits(channel as u32, 4, &mut timings, &mut idx);
        Self::trbits(command as u32, 4, &mut timings, &mut idx);
        Self::trbits(strength as u32, 8, &mut timings, &mut idx);
        Self::trbits(checksum as u32, 8, &mut timings, &mut idx);
        Self::trbits(0, 2, &mut timings, &mut idx);

        timings[idx] = ZERO_LEN;
        idx += 1;
        timings[idx] = 1476;
        idx += 1;
        timings[idx] = 0;

        let duration = duration.into();
        let end = (self.now_fn)() + duration;
        while (self.now_fn)() < end {
            self.send_timing(&timings[..idx]);
        }
    }

    fn send_timing(&mut self, timings: &[u16]) {
        let mut level = false;
        for &duration in timings.iter().take_while(|&&t| t != 0) {
            let _ = if level {
                self.pin.set_high()
            } else {
                self.pin.set_low()
            };
            self.delay.borrow_mut().delay_us(duration);
            level = !level;
        }
        let _ = self.pin.set_low();
    }

    fn trbits(val: u32, bits: u8, timings: &mut [u16], idx: &mut usize) {
        for i in (0..bits).rev() {
            let bit_set = (val >> i) & 1 != 0;
            let len = if bit_set { ONE_LEN } else { ZERO_LEN };
            timings[*idx] = len;
            *idx += 1;
            timings[*idx] = PULSE_LEN - len;
            *idx += 1;
        }
    }
}
