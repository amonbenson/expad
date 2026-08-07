use super::{Address, RegisterSpec};

/// Read-only chip identification register. The upper nibble holds the part
/// ID; the lower nibble encodes silicon revision.
#[derive(Clone, Copy, Debug, Default)]
pub struct Id(u8);

impl RegisterSpec for Id {
    const ADDRESS: Address = Address::Id;

    fn bits(&self) -> u8 {
        self.0
    }

    fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

impl Id {
    pub fn chip_id(self) -> u8 {
        self.0 & 0xf0
    }
}
