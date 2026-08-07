use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, MisoPin, Spi};

mod registers;

use registers::Register;
use registers::id::Id;
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

pub struct AdcChain<'d, const N: usize> {
    spi: Spi<'d, SPI0, Blocking>,
    cs: [Output<'d>; N],
    rdy: [Input<'d>; N],
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

        Self { spi, cs, rdy }
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
        control.set_channel_configuration(ChannelConfiguration::single(1).unwrap());
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

        Ok(())
    }
}
