use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, Spi};
use embassy_time::{Duration, block_for};

/// Width of the latch pulse. The 74HC595 needs a few dozen ns at 3.3 V, which back-to-back
/// GPIO writes do not reliably guarantee.
const LATCH_PULSE_WIDTH: Duration = Duration::from_micros(1);

/// A chain of `N` 74HC595 shift registers on SPI1.
///
/// The outputs stay disabled (the board pulls OE high) until the first write, so whatever
/// the registers power up with never reaches the switches they drive. From then on they stay
/// enabled: every write shifts into the storage registers and latches all outputs at once,
/// so there is no need to disable them while shifting.
pub struct ShiftRegisterChain<'d, const N: usize> {
    spi: Spi<'d, SPI1, Blocking>,
    oe: Output<'d>,
    lat: Output<'d>,
}

impl<'d, const N: usize> ShiftRegisterChain<'d, N> {
    pub fn new(
        spi: embassy_rp::Peri<'d, SPI1>,
        clk: embassy_rp::Peri<'d, impl ClkPin<SPI1> + 'd>,
        data: embassy_rp::Peri<'d, impl MosiPin<SPI1> + 'd>,
        oe: embassy_rp::Peri<'d, impl Pin>,
        lat: embassy_rp::Peri<'d, impl Pin>,
    ) -> Self {
        let spi = Spi::new_blocking_txonly(spi, clk, data, Config::default());
        let oe = Output::new(oe, Level::High);
        let lat = Output::new(lat, Level::Low);

        Self { spi, oe, lat }
    }

    /// Shifts `data` out, first byte to the last chip in the chain, and latches it onto every
    /// chip's outputs at once.
    pub fn write(&mut self, data: [u8; N]) -> Result<(), Error> {
        self.spi.blocking_write(&data)?;

        self.lat.set_high();
        block_for(LATCH_PULSE_WIDTH);
        self.lat.set_low();
        self.oe.set_low();

        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), Error> {
        self.write([0; N])
    }
}
