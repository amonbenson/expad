use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, MisoPin, Spi};

mod registers;

use registers::id::Id;
use registers::mode::{AdcMode, ChannelCount, Mode, Reference};
use registers::{Address, RegisterSpec};

const AD7718_ID: u8 = 0x40;

#[derive(Debug)]
pub enum AdcError {
    Spi(Error),
    IdMismatch {
        chip: u8,
        expected_id: u8,
        actual_id: u8,
    },
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

    fn control_byte(operation: Operation, address: Address) -> u8 {
        (operation as u8) << 6 | (address as u8) & 0x0f
    }

    pub fn write_register<R: RegisterSpec>(&mut self, chip: usize, register: R) -> Result<(), AdcError> {
        let data = [Self::control_byte(Operation::Write, R::ADDRESS), register.bits()];

        self.cs[chip].set_low();
        self.spi.blocking_write(&data).map_err(AdcError::Spi)?;
        self.cs[chip].set_high();

        Ok(())
    }

    pub fn read_register<R: RegisterSpec>(&mut self, chip: usize) -> Result<R, AdcError> {
        let mut data = [Self::control_byte(Operation::Read, R::ADDRESS), 0];

        self.cs[chip].set_low();
        self.spi.blocking_transfer_in_place(&mut data).map_err(AdcError::Spi)?;
        self.cs[chip].set_high();

        Ok(R::from_bits(data[1]))
    }

    pub fn write_all_registers<R: RegisterSpec + Copy>(&mut self, register: R) -> Result<(), AdcError> {
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

    pub fn init(&mut self) -> Result<(), AdcError> {
        self.ensure_connected()?;

        let mut mode = Mode::default();
        mode.set_chopping(true);
        mode.set_negative_buffer(false);
        mode.set_reference(Reference::RefIn1);
        mode.set_channel_count(ChannelCount::Eight);
        mode.set_oscillator_power_down(false);
        mode.set_adc_mode(AdcMode::Idle);
        self.write_all_registers(mode)?;

        Ok(())
    }
}
