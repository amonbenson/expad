use super::{Address, Field, Register, RegisterSpec};

#[derive(Clone, Copy, Debug, Default)]
pub struct Status(Register);

impl RegisterSpec for Status {
    const ADDRESS: Address = Address::Status;

    fn bits(&self) -> u8 {
        self.0.bits()
    }

    fn from_bits(bits: u8) -> Self {
        Self(Register::new(bits))
    }
}

impl Status {
    const RDY: Field = Field::new(7, 0b1);
    const CAL: Field = Field::new(5, 0b1);
    const ERR: Field = Field::new(3, 0b1);
    const LOCK: Field = Field::new(0, 0b1);

    pub fn ready(&self) -> bool {
        !self.0.bit(Self::RDY)
    }

    pub fn calibration(&self) -> bool {
        !self.0.bit(Self::CAL)
    }

    pub fn error(&self) -> bool {
        !self.0.bit(Self::ERR)
    }

    pub fn lock(&self) -> bool {
        self.0.bit(Self::LOCK)
    }
}
