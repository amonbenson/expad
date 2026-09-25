use embassy_futures::join::join_array;
use embassy_futures::select::{select, select_array};
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{self, Blocking, ClkPin, Config, MisoPin, MosiPin, Spi};
use embassy_time::{Duration, Timer, block_for};

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

/// Reference voltage the AD7718's gain ranges are specified against. The actual full scale
/// scales with whatever reference is applied to REFIN, so a board running a different one
/// reads every range proportionally wider or narrower.
const NOMINAL_REFERENCE_VOLTAGE: f32 = 2.5;

/// Widest register on the wire (the 24-bit Data register), in bytes.
const MAX_REGISTER_WIDTH: usize = 3;

/// Time to wait after a soft reset before the registers may be accessed again.
const RESET_DURATION: Duration = Duration::from_millis(1);

/// Time chip select is held around every transaction. Found necessary on the breadboard once
/// the driver runs optimized: with 2 µs the ADC loses sync with the transfers, and with 10 µs
/// it stays in sync but noticeably more conversions land on outlying codes. 50 µs is clean
/// and still small next to a conversion.
const CHIP_SELECT_GUARD: Duration = Duration::from_micros(50);

/// Longest the ADC takes to raise RDY after being given a new conversion or calibration.
/// Its logic runs from a 32.768 kHz crystal, so this is a few dozen clock cycles.
const COMMAND_PICKUP_TIMEOUT: Duration = Duration::from_millis(1);

#[derive(Debug, Clone, Copy, defmt::Format)]
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
    reference_voltage: f32,
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
            reference_voltage: NOMINAL_REFERENCE_VOLTAGE,
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

    /// Sets the conversion rate in Hz, rounded to the nearest one the filter supports. Slower
    /// rates average over a longer window and so resolve far finer steps; the fastest one
    /// leaves only around 11 noise-free bits.
    pub fn with_update_rate(mut self, update_rate: u32) -> Self {
        self.update_rate = update_rate;
        self
    }

    /// Sets the voltage actually applied to the selected reference input, which every
    /// range is scaled by when a code is converted back into a voltage.
    pub fn with_reference_voltage(mut self, reference_voltage: f32) -> Self {
        self.reference_voltage = reference_voltage;
        self
    }

    /// Voltage at the top of the selected range, given the reference actually applied.
    fn full_scale_voltage(&self) -> f32 {
        self.range.voltage() * (self.reference_voltage / NOMINAL_REFERENCE_VOLTAGE)
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

    fn select(&mut self, chip: usize) {
        self.cs[chip].set_low();
        block_for(CHIP_SELECT_GUARD);
    }

    fn deselect(&mut self, chip: usize) {
        block_for(CHIP_SELECT_GUARD);
        self.cs[chip].set_high();
        block_for(CHIP_SELECT_GUARD);
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

        self.select(chip);
        let result = self.spi.blocking_write(data);
        self.deselect(chip);

        result.map_err(AdcChainError::Spi)
    }

    pub fn read_register<R: Register>(&mut self, chip: usize) -> Result<R, AdcChainError> {
        let mut buf = [0u8; 1 + MAX_REGISTER_WIDTH];
        buf[0] = Self::control_byte(Operation::Read, R::ADDRESS);
        let data = &mut buf[..1 + R::WIDTH];

        self.select(chip);
        let result = self.spi.blocking_transfer_in_place(data);
        self.deselect(chip);
        result.map_err(AdcChainError::Spi)?;

        let bits = data[1..]
            .iter()
            .fold(0u32, |bits, &byte| (bits << 8) | byte as u32);

        Ok(R::from_bits(bits))
    }

    fn soft_reset(&mut self, chip: usize) -> Result<(), AdcChainError> {
        let reset = [0xFFu8; 4];

        // Clock out 32 ones to reset the ADC (as described in the datasheet)
        self.select(chip);
        let result = self.spi.blocking_write(&reset);
        self.deselect(chip);
        result.map_err(AdcChainError::Spi)?;

        // The registers are not accessible until the reset has completed internally.
        // Unoptimized builds used to spend long enough getting to the next access that
        // this went unnoticed.
        block_for(RESET_DURATION);

        Ok(())
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
                join_array(self.rdy.each_mut().map(wait_for_completion)).await;
            }
        }

        Ok(())
    }

    fn code_to_voltage(&self, value: u32) -> f32 {
        let full_scale = self.config.full_scale_voltage();

        match self.config.coding {
            Coding::Unipolar => (value as f32 / 0xFFFFFF as f32) * full_scale,
            Coding::Bipolar => ((value as i32 - 0x800000) as f32 / 0x7FFFFF as f32) * full_scale,
        }
    }

    /// Switches every chip to the filter setting closest to `update_rate`, keeping the
    /// calibration taken at the previous one. Offset and gain can shift slightly between
    /// filter settings, so readings meant to be compared should share one setting.
    pub fn set_update_rate(&mut self, update_rate: u32) -> Result<(), AdcChainError> {
        self.config = self.config.with_update_rate(update_rate);
        let filter = self.config.filter_register();
        self.write_all_registers(filter)
    }

    /// Conversion rate the chips actually run at, after rounding to the filter's steps.
    pub fn update_rate(&self) -> u32 {
        self.config
            .filter_register()
            .update_rate(self.config.chopping)
    }

    /// Converts `channel` continuously and fills `voltages` with consecutive results, then
    /// leaves the chip idle. Staying on one channel, the ADC delivers a result every
    /// conversion period instead of paying its filter's full settling time for each one;
    /// only the first result waits for that settling.
    pub async fn measure_continuous(
        &mut self,
        chip: usize,
        channel: u8,
        voltages: &mut [f32],
    ) -> Result<(), AdcChainError> {
        let control = self.config.control_register(channel)?;
        self.write_register(chip, control)?;
        let mode = self.config.mode_register(AdcMode::ContinuousConversion);
        self.write_register(chip, mode)?;

        for voltage in voltages.iter_mut() {
            // Reading the data register raises RDY again, so only the first result can
            // find it still low from before.
            wait_for_completion(&mut self.rdy[chip]).await;
            let data: Data = self.read_register(chip)?;
            *voltage = self.code_to_voltage(data.bits());
        }

        let idle = self.config.mode_register(AdcMode::Idle);
        self.write_register(chip, idle)
    }

    /// Voltage a reading of full scale corresponds to, which is also the highest voltage
    /// this chain can tell apart from anything above it.
    pub fn full_scale_voltage(&self) -> f32 {
        self.config.full_scale_voltage()
    }

    /// Starts one conversion of `channel`. The filter register is left as `init` wrote it,
    /// since it never changes afterwards.
    fn start_single_conversion(&mut self, chip: usize, channel: u8) -> Result<(), AdcChainError> {
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
        wait_for_completion(&mut self.rdy[chip]).await;

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

/// Waits for the conversion or calibration just started to finish.
///
/// RDY only rises once the ADC has picked the new command up, which takes tens of µs at its
/// 32.768 kHz clock. Waiting for it to fall straight away returns immediately whenever it is
/// still low from before - which it always is after a calibration, whose result is never
/// read - and hands back a result that does not exist yet.
async fn wait_for_completion(rdy: &mut Input<'_>) {
    select(rdy.wait_for_high(), Timer::after(COMMAND_PICKUP_TIMEOUT)).await;
    rdy.wait_for_low().await;
}
