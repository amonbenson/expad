use embassy_futures::select::select_array;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, MisoPin, Spi};

mod registers;

use registers::Register;
use registers::id::Id;
use registers::data::Data;
use registers::mode::{AdcMode, ChannelCount, Mode, Reference};
use registers::control::{ChannelConfiguration, Coding, Control, Range};

const AD7718_ID: u8 = 0x40;

/// Widest register on the wire (the 24-bit Data register), in bytes.
const MAX_REGISTER_WIDTH: usize = 3;

#[derive(Debug)]
pub enum AdcError {
    Spi(Error),
    IdMismatch {
        chip: u8,
        expected_id: u8,
        actual_id: u8,
    },
    InvalidChannel {
        chip: u8,
        channel: u8,
    },
    ContinuousMeasurementNotRunning,
}

pub struct AdcChainConfig {
    chopping: bool,
    negative_buffer: bool,
    reference: Reference,
    channel_count: ChannelCount,
    oscillator_power_down: bool,
    coding: Coding,
    range: Range,
}

impl Default for AdcChainConfig {
    fn default() -> Self {
        Self {
            chopping: true,
            negative_buffer: false,
            reference: Reference::RefIn1,
            channel_count: ChannelCount::Eight,
            oscillator_power_down: false,
            coding: Coding::Unipolar,
            range: Range::Range2_56V,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum Operation {
    Write = 0b0,
    Read = 0b1,
}

/// A single-ended reading taken from one channel of one chip in the chain.
#[derive(Clone, Copy, Debug)]
pub struct Measurement {
    pub chip: u8,
    pub channel: u8,
    pub value: u32,
}

pub struct AdcChain<'d, const N: usize> {
    spi: Spi<'d, SPI0, Blocking>,
    cs: [Output<'d>; N],
    rdy: [Input<'d>; N],
    channel_count: ChannelCount,
    continuous_capture: Option<[u8; N]>,
}

impl<'d, const N: usize> AdcChain<'d, N> {
    pub fn new(
        spi: embassy_rp::Peri<'d, SPI0>,
        clk: embassy_rp::Peri<'d, impl ClkPin<SPI0> + 'd>,
        tx: embassy_rp::Peri<'d, impl MosiPin<SPI0> + 'd>,
        rx: embassy_rp::Peri<'d, impl MisoPin<SPI0> + 'd>,
        cs: [embassy_rp::Peri<'d, impl Pin>; N],
        rdy: [embassy_rp::Peri<'d, impl Pin>; N],
    ) -> Self {
        let spi = Spi::new_blocking(spi, clk, tx, rx, Config::default());
        let cs = cs.map(|c| Output::new(c, Level::High));
        let rdy = rdy.map(|r| Input::new(r, Pull::Up));

        Self {
            spi,
            cs,
            rdy,
            channel_count: ChannelCount::Eight,
            continuous_capture: None,
        }
    }

    fn control_byte(operation: Operation, address: u8) -> u8 {
        (operation as u8) << 6 | address & 0x0f
    }

    pub fn write_register<R: Register>(&mut self, chip: usize, register: R) -> Result<(), AdcError> {
        let mut buf = [0u8; 1 + MAX_REGISTER_WIDTH];
        buf[0] = Self::control_byte(Operation::Write, R::ADDRESS);

        let bits = register.bits();
        for (i, byte) in buf[1..1 + R::WIDTH].iter_mut().enumerate() {
            *byte = (bits >> (8 * (R::WIDTH - 1 - i))) as u8;
        }
        let data = &buf[..1 + R::WIDTH];

        self.cs[chip].set_low();
        self.spi.blocking_write(data).map_err(AdcError::Spi)?;
        self.cs[chip].set_high();

        Ok(())
    }

    pub fn read_register<R: Register>(&mut self, chip: usize) -> Result<R, AdcError> {
        let mut buf = [0u8; 1 + MAX_REGISTER_WIDTH];
        buf[0] = Self::control_byte(Operation::Read, R::ADDRESS);
        let data = &mut buf[..1 + R::WIDTH];

        self.cs[chip].set_low();
        self.spi.blocking_transfer_in_place(data).map_err(AdcError::Spi)?;
        self.cs[chip].set_high();

        let bits = data[1..].iter().fold(0u32, |bits, &byte| (bits << 8) | byte as u32);

        Ok(R::from_bits(bits))
    }

    pub fn write_all_registers<R: Register + Copy>(&mut self, register: R) -> Result<(), AdcError> {
        for chip in 0..N {
            self.write_register(chip, register)?;
        }

        Ok(())
    }

    pub fn ensure_connected(&mut self) -> Result<(), AdcError> {
        for chip in 0..N {
            let id: Id = self.read_register(chip)?;

            if id.chip_id() != AD7718_ID {
                return Err(AdcError::IdMismatch {
                    chip: chip as u8,
                    expected_id: AD7718_ID,
                    actual_id: id.chip_id(),
                });
            }
        }

        Ok(())
    }

    pub fn connected(&mut self) -> bool {
        self.ensure_connected().is_ok()
    }

    pub fn init(&mut self, config: AdcChainConfig) -> Result<(), AdcError> {
        self.ensure_connected()?;

        let mut control = Control::default();
        control.set_channel_configuration(ChannelConfiguration::single(1, config.channel_count).unwrap());
        control.set_coding(config.coding);
        control.set_range(config.range);
        self.write_all_registers(control)?;

        let mut mode = Mode::default();
        mode.set_chopping(config.chopping);
        mode.set_negative_buffer(config.negative_buffer);
        mode.set_reference(config.reference);
        mode.set_channel_count(config.channel_count);
        mode.set_oscillator_power_down(config.oscillator_power_down);
        mode.set_adc_mode(AdcMode::Idle);
        self.write_all_registers(mode)?;

        self.channel_count = config.channel_count;

        Ok(())
    }

    fn start_single_conversion(&mut self, chip: usize, channel: u8) -> Result<(), AdcError> {
        let channel_configuration =
            ChannelConfiguration::single(channel, self.channel_count)
                .ok_or(AdcError::InvalidChannel {
                    chip: chip as u8,
                    channel,
                })?;

        let mut control = self.read_register::<Control>(chip)?;
        control.set_channel_configuration(channel_configuration);
        self.write_register(chip, control)?;

        let mut mode = self.read_register::<Mode>(chip)?;
        mode.set_adc_mode(AdcMode::SingleConversion);
        self.write_register(chip, mode)?;

        Ok(())
    }

    fn next_channel(&self, channel: u8) -> u8 {
        if channel < self.channel_count.count() {
            channel + 1
        } else {
            1
        }
    }

    pub async fn measure_channel(&mut self, chip: usize, channel: u8) -> Result<u32, AdcError> {
        self.start_single_conversion(chip, channel)?;
        self.rdy[chip].wait_for_low().await;

        let data: Data = self.read_register(chip)?;
        Ok(data.bits())
    }

    pub fn start_continuous_capture(&mut self) -> Result<(), AdcError> {
        for chip in 0..N {
            self.start_single_conversion(chip, 1)?;
        }
        self.continuous_capture = Some([1; N]);

        Ok(())
    }

    pub fn stop_continuous_capture(&mut self) {
        self.continuous_capture = None;
    }

    pub fn continuous_capture_active(&self) -> bool {
        self.continuous_capture.is_some()
    }

    pub async fn wait_for_next_result(&mut self) -> Result<Measurement, AdcError> {
        if !self.continuous_capture_active() {
            return Err(AdcError::ContinuousMeasurementNotRunning);
        }

        let (_, chip) = select_array(self.rdy.each_mut().map(Input::wait_for_low)).await;

        let Some(channels) = &mut self.continuous_capture else {
            return Err(AdcError::ContinuousMeasurementNotRunning);
        };
        let channel = channels[chip];
        let next_channel = self.next_channel(channel);

        let data: Data = self.read_register(chip)?;
        let value = data.bits();

        self.start_single_conversion(chip, next_channel)?;
        if let Some(channels) = &mut self.continuous_capture {
            channels[chip] = next_channel;
        }

        Ok(Measurement {
            chip: chip as u8,
            channel,
            value,
        })
    }
}
