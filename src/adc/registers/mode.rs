use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::RegisterValue;

pub type Mode = RegisterValue<0x01, 1>;

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum Reference {
    RefIn1 = 0b0,
    RefIn2 = 0b1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum ChannelCount {
    Eight = 0b0,
    Ten = 0b1,
}

impl ChannelCount {
    pub const fn new(count: u8) -> Option<Self> {
        match count {
            8 => Some(Self::Eight),
            10 => Some(Self::Ten),
            _ => None,
        }
    }

    pub const fn count(self) -> u8 {
        match self {
            Self::Eight => 8,
            Self::Ten => 10,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum AdcMode {
    PowerDown = 0b000,
    Idle = 0b001,
    SingleConversion = 0b010,
    ContinuousConversion = 0b011,
    InternalZeroScaleCalibration = 0b100,
    InternalFullScaleCalibration = 0b101,
    SystemZeroScaleCalibration = 0b110,
    SystemFullScaleCalibration = 0b111,
}

impl Mode {
    pub fn chopping(&self) -> bool {
        self.bit_active_low(7)
    }

    pub fn set_chopping(&mut self, value: bool) {
        self.set_bit_active_low(7, value);
    }

    pub fn negative_buffer(&self) -> bool {
        self.bit(6)
    }

    pub fn set_negative_buffer(&mut self, value: bool) {
        self.set_bit(6, value);
    }

    pub fn reference(&self) -> Reference {
        self.field(5, 1)
    }

    pub fn set_reference(&mut self, value: Reference) {
        self.set_field(5, 1, value);
    }

    pub fn channel_count(&self) -> ChannelCount {
        self.field(4, 1)
    }

    pub fn set_channel_count(&mut self, value: ChannelCount) {
        self.set_field(4, 1, value);
    }

    pub fn oscillator_power_down(&self) -> bool {
        self.bit(3)
    }

    pub fn set_oscillator_power_down(&mut self, value: bool) {
        self.set_bit(3, value);
    }

    pub fn adc_mode(&self) -> AdcMode {
        self.field(0, 3)
    }

    pub fn set_adc_mode(&mut self, value: AdcMode) {
        self.set_field(0, 3, value);
    }
}
