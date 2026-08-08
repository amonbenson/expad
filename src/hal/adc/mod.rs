use embassy_futures::join::join_array;
use embassy_futures::select::select_array;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{self, Blocking, ClkPin, Config, MisoPin, MosiPin, Spi};

mod registers;

use registers::Register;
use registers::control::{ChannelConfiguration, Control};
use registers::data::Data;
use registers::filter::Filter;
use registers::id::Id;
use registers::mode::{AdcMode, Mode, Reference};

pub use registers::control::{Coding, Range};
pub use registers::mode::ChannelCount;

const AD7718_ID: u8 = 0x40;

/// Widest register on the wire (the 24-bit Data register), in bytes.
const MAX_REGISTER_WIDTH: usize = 3;

#[derive(Debug, Clone, Copy)]
pub enum AdcChainError {
    Spi(spi::Error),
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

#[derive(Clone, Copy)]
pub struct AdcChainConfig {
    chopping: bool,
    negative_buffer: bool,
    reference: Reference,
    channel_count: ChannelCount,
    oscillator_power_down: bool,
    coding: Coding,
    range: Range,
    update_rate: u32,
}

impl Default for AdcChainConfig {
    fn default() -> Self {
        let chopping = false;
        let update_rate = Filter::best_speed(chopping).update_rate(chopping);

        Self {
            chopping,
            negative_buffer: false,
            reference: Reference::RefIn1,
            channel_count: ChannelCount::Eight,
            oscillator_power_down: false,
            coding: Coding::Unipolar,
            range: Range::V2_56V,
            update_rate,
        }
    }
}

impl AdcChainConfig {
    pub fn with_channel_count(mut self, channel_count: ChannelCount) -> Self {
        self.channel_count = channel_count;
        self
    }

    /// Largest full-scale range the AD7718 supports is `Range::V2_56V`; inputs above
    /// that (relative to VREF) will saturate no matter which range is selected here.
    pub fn with_range(mut self, range: Range) -> Self {
        self.range = range;
        self
    }

    pub fn with_coding(mut self, coding: Coding) -> Self {
        self.coding = coding;
        self
    }

    fn filter_register(&self) -> Filter {
        Filter::from_update_rate(self.update_rate, self.chopping)
    }

    fn control_register(&self, channel: u8) -> Result<Control, AdcChainError> {
        let channel_configuration = ChannelConfiguration::single(channel, self.channel_count)
            .ok_or(AdcChainError::InvalidChannel { chip: 0, channel })?;

        let mut control = Control::default();
        control.set_channel_configuration(channel_configuration);
        control.set_coding(self.coding);
        control.set_range(self.range);

        Ok(control)
    }

    fn mode_register(&self, adc_mode: AdcMode) -> Mode {
        let mut mode = Mode::default();
        mode.set_chopping(self.chopping);
        mode.set_negative_buffer(self.negative_buffer);
        mode.set_reference(self.reference);
        mode.set_channel_count(self.channel_count);
        mode.set_oscillator_power_down(self.oscillator_power_down);
        mode.set_adc_mode(adc_mode);
        mode
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
    pub voltage: f32,
}

pub struct AdcChain<'d, const N: usize> {
    spi: Spi<'d, SPI0, Blocking>,
    cs: [Output<'d>; N],
    rdy: [Input<'d>; N],
    config: AdcChainConfig,
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
            config: AdcChainConfig::default(),
            continuous_capture: None,
        }
    }

    fn control_byte(operation: Operation, address: u8) -> u8 {
        (operation as u8) << 6 | address & 0x0f
    }

    pub fn write_register<R: Register>(
        &mut self,
        chip: usize,
        register: R,
    ) -> Result<(), AdcChainError> {
        let mut buf = [0u8; 1 + MAX_REGISTER_WIDTH];
        buf[0] = Self::control_byte(Operation::Write, R::ADDRESS);

        let bits = register.bits();
        for (i, byte) in buf[1..1 + R::WIDTH].iter_mut().enumerate() {
            *byte = (bits >> (8 * (R::WIDTH - 1 - i))) as u8;
        }
        let data = &buf[..1 + R::WIDTH];

        self.cs[chip].set_low();
        let result = self.spi.blocking_write(data);
        self.cs[chip].set_high();

        result.map_err(AdcChainError::Spi)
    }

    pub fn read_register<R: Register>(&mut self, chip: usize) -> Result<R, AdcChainError> {
        let mut buf = [0u8; 1 + MAX_REGISTER_WIDTH];
        buf[0] = Self::control_byte(Operation::Read, R::ADDRESS);
        let data = &mut buf[..1 + R::WIDTH];

        self.cs[chip].set_low();
        let result = self.spi.blocking_transfer_in_place(data);
        self.cs[chip].set_high();
        result.map_err(AdcChainError::Spi)?;

        let bits = data[1..]
            .iter()
            .fold(0u32, |bits, &byte| (bits << 8) | byte as u32);

        Ok(R::from_bits(bits))
    }

    fn soft_reset(&mut self, chip: usize) -> Result<(), AdcChainError> {
        let reset = [0xFFu8; 4];

        // Clock out 32 ones to reset the ADC (as described in the datasheet)
        self.cs[chip].set_low();
        let result = self.spi.blocking_write(&reset);
        self.cs[chip].set_high();

        result.map_err(AdcChainError::Spi)
    }

    pub fn write_all_registers<R: Register + Copy>(
        &mut self,
        register: R,
    ) -> Result<(), AdcChainError> {
        for chip in 0..N {
            self.write_register(chip, register)?;
        }

        Ok(())
    }

    pub fn ensure_connected(&mut self) -> Result<(), AdcChainError> {
        for chip in 0..N {
            self.soft_reset(chip)?;

            let id: Id = self.read_register(chip)?;

            if id.chip_id() != AD7718_ID {
                return Err(AdcChainError::IdMismatch {
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

    pub async fn init(&mut self, config: AdcChainConfig) -> Result<(), AdcChainError> {
        self.ensure_connected()?;
        self.config = config;

        let filter = self.config.filter_register();
        self.write_all_registers(filter)?;

        let control = self.config.control_register(0)?;
        self.write_all_registers(control)?;

        let mode = self.config.mode_register(AdcMode::Idle);
        self.write_all_registers(mode)?;

        self.calibrate().await?;

        Ok(())
    }

    pub async fn calibrate(&mut self) -> Result<(), AdcChainError> {
        // Cal registers are shared in pairs across single-ended channels, so
        // calibrating the first half of channels covers every group.
        let calibration_groups = self.config.channel_count.count() / 2;

        // Calibrate each channel group sequentially
        for channel in 0..calibration_groups {
            let control = self.config.control_register(channel)?;
            for chip in 0..N {
                self.write_register(chip, control)?;
            }

            // Calibrate zero-scale (offset) first, then full-scale (gain)
            for calibration_mode in [
                AdcMode::InternalZeroScaleCalibration,
                AdcMode::InternalFullScaleCalibration,
            ] {
                let mode = self.config.mode_register(calibration_mode);
                for chip in 0..N {
                    self.write_register(chip, mode)?;
                }

                // Wait for all chips to finish calibration in parallel
                join_array(self.rdy.each_mut().map(Input::wait_for_low)).await;
            }
        }

        Ok(())
    }

    fn code_to_voltage(&self, value: u32) -> f32 {
        let range = self.config.range.voltage();
        let coding = self.config.coding;

        // TODO: verify this
        match coding {
            Coding::Unipolar => (value as f32 / 0xFFFFFF as f32) * range,
            Coding::Bipolar => ((value as i32 - 0x800000) as f32 / 0x7FFFFF as f32) * range,
        }
    }

    fn start_single_conversion(&mut self, chip: usize, channel: u8) -> Result<(), AdcChainError> {
        let filter = self.config.filter_register();
        self.write_register(chip, filter)?;

        let control = self.config.control_register(channel)?;
        self.write_register(chip, control)?;

        let mode = self.config.mode_register(AdcMode::SingleConversion);
        self.write_register(chip, mode)?;

        Ok(())
    }

    fn next_channel(&self, channel: u8) -> u8 {
        if channel + 1 < self.config.channel_count.count() {
            channel + 1
        } else {
            0
        }
    }

    pub async fn measure_channel(
        &mut self,
        chip: usize,
        channel: u8,
    ) -> Result<f32, AdcChainError> {
        self.start_single_conversion(chip, channel)?;
        self.rdy[chip].wait_for_low().await;

        let data: Data = self.read_register(chip)?;
        Ok(self.code_to_voltage(data.bits()))
    }

    pub fn start_continuous_capture(&mut self) -> Result<(), AdcChainError> {
        for chip in 0..N {
            self.start_single_conversion(chip, 0)?;
        }
        self.continuous_capture = Some([0; N]);

        Ok(())
    }

    pub fn stop_continuous_capture(&mut self) {
        self.continuous_capture = None;
    }

    pub fn continuous_capture_active(&self) -> bool {
        self.continuous_capture.is_some()
    }

    pub async fn wait_for_next_result(&mut self) -> Result<Measurement, AdcChainError> {
        if !self.continuous_capture_active() {
            return Err(AdcChainError::ContinuousMeasurementNotRunning);
        }

        let (_, chip) = select_array(self.rdy.each_mut().map(Input::wait_for_low)).await;

        let channel = self
            .continuous_capture
            .ok_or(AdcChainError::ContinuousMeasurementNotRunning)?[chip];
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
            voltage: self.code_to_voltage(value),
        })
    }
}
