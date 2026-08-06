use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::{Blocking, ClkPin, Config, Error, MosiPin, MisoPin, Spi};

pub const AD7718_ID: u8 = 0x40;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterOperation {
    Write = 0b0,
    Read = 0b1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterAddress {
    Status = 0x00,
    Mode = 0x01,
    Control = 0x02,
    Filter = 0x03,
    Data = 0x04,
    Offset = 0x05,
    Gain = 0x06,
    IOControl = 0x07,
    Test1 = 0x0c,
    Test2 = 0x0d,
    ID = 0x0f,
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

    pub fn pack_cr(operation: RegisterOperation, address: RegisterAddress) -> u8 {
        (operation as u8) << 6 | (address as u8) & 0x0f
    }

    pub fn write_register(&mut self, chip: usize, address: RegisterAddress, value: u8) -> Result<(), Error> {
        let cr = Self::pack_cr(RegisterOperation::Write, address);
        let data = [cr, value];

        self.cs[chip].set_low();
        self.spi.blocking_write(&data)?;
        self.cs[chip].set_high();

        Ok(())
    }

    pub fn read_register(&mut self, chip: usize, address: RegisterAddress) -> Result<u8, Error> {
        let cr = Self::pack_cr(RegisterOperation::Read, address);
        let mut data = [cr, 0];

        self.cs[chip].set_low();
        self.spi.blocking_transfer_in_place(&mut data)?;
        self.cs[chip].set_high();

        Ok(data[1])
    }

    pub fn write_all_registers(&mut self, address: RegisterAddress, value: u8) -> Result<(), Error> {
        for chip in 0..N {
            self.write_register(chip, address, value)?;
        }

        Ok(())
    }

    pub fn read_all_registers(&mut self, address: RegisterAddress) -> Result<[u8; N], Error> {
        let mut values = [0u8; N];

        for chip in 0..N {
            values[chip] = self.read_register(chip, address)?;
        }

        Ok(values)
    }

    pub fn chip_connected(&mut self, chip: usize) -> Result<bool, Error> {
        let id = self.read_register(chip, RegisterAddress::ID)?;
        Ok((id & 0xF0) == AD7718_ID)
    }

    pub fn connected(&mut self) -> bool {
        (0..N).all(|chip| self.chip_connected(chip).unwrap_or(false))
    }
}
