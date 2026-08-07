use super::{Address, Field, Register, RegisterSpec};

#[derive(Clone, Copy, Debug, Default)]
pub struct Id(Register);

impl RegisterSpec for Id {
    const ADDRESS: Address = Address::Id;

    fn bits(&self) -> u8 {
        self.0.bits()
    }

    fn from_bits(bits: u8) -> Self {
        Self(Register::new(bits))
    }
}

impl Id {
    const CHIP_ID: Field = Field::new(0, 0xff);

    pub fn chip_id(self) -> u8 {
        // the lower nibble should be ignored according to the datasheet, so mask it out
        self.0.field(Self::CHIP_ID) & 0xf0
    }
}
