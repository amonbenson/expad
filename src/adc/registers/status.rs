use super::RegisterValue;

pub type Status = RegisterValue<0x00, 1>;

impl Status {
    pub fn ready(&self) -> bool {
        self.bit_active_low(7)
    }

    pub fn calibration(&self) -> bool {
        self.bit_active_low(5)
    }

    pub fn error(&self) -> bool {
        self.bit_active_low(3)
    }

    pub fn lock(&self) -> bool {
        self.bit(0)
    }
}
