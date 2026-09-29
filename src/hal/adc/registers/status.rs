use super::RegisterValue;

pub type Status = RegisterValue<0x00, 1>;

impl Status {
    /// Set once a result or calibration has completed; the RDY pin is its complement.
    pub fn ready(&self) -> bool {
        self.bit(7)
    }

    /// Set once a calibration has completed.
    pub fn calibration(&self) -> bool {
        self.bit(5)
    }

    /// Set when the last result was clamped (overrange) or a calibration failed.
    pub fn error(&self) -> bool {
        self.bit(3)
    }

    /// Set while the PLL is locked to the 32.768 kHz crystal.
    pub fn lock(&self) -> bool {
        self.bit(0)
    }
}
