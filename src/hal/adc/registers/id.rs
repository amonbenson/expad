use super::{Register, RegisterValue};

pub type Id = RegisterValue<0x0f, 1>;

impl Id {
    pub fn chip_id(self) -> u8 {
        // the lower nibble should be ignored according to the datasheet, so mask it out
        (self.bits() & 0xf0) as u8
    }
}
