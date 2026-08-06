use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, Spi};

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
        let mut oe = Output::new(oe, Level::High);
        let mut lat = Output::new(lat, Level::Low);

        oe.set_high();
        lat.set_low();

        Self { spi, oe, lat }
    }

    pub fn write(&mut self, data: [u8; N]) -> Result<(), Error> {
        self.oe.set_high();
        self.lat.set_low();

        self.spi.blocking_write(&data)?;

        self.lat.set_high();
        self.lat.set_low();
        self.oe.set_low();

        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), Error> {
        self.write([0; N])
    }
}