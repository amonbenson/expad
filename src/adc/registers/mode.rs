use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{Address, Field, Register, RegisterSpec};

#[derive(Clone, Copy, Debug, Default)]
pub struct Mode(Register);

impl RegisterSpec for Mode {
    const ADDRESS: Address = Address::Mode;

    fn bits(&self) -> u8 {
        self.0.bits()
    }

    fn from_bits(bits: u8) -> Self {
        Self(Register::new(bits))
    }
}

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
    const NCHOP: Field = Field::new(7, 0b1);
    const NEGBUF: Field = Field::new(6, 0b1);
    const REFSEL: Field = Field::new(5, 0b1);
    const CHCON: Field = Field::new(4, 0b1);
    const OSCPD: Field = Field::new(3, 0b1);
    const ADC_MODE: Field = Field::new(0, 0b111);

    pub fn chopping(&self) -> bool {
        !self.0.bit(Self::NCHOP)
    }

    pub fn set_chopping(&mut self, enabled: bool) {
        self.0.set_bit(Self::NCHOP, !enabled);
    }

    pub fn negative_buffer(&self) -> bool {
        self.0.bit(Self::NEGBUF)
    }

    pub fn set_negative_buffer(&mut self, enabled: bool) {
        self.0.set_bit(Self::NEGBUF, enabled);
    }

    pub fn reference(&self) -> Reference {
        Reference::try_from(self.0.field(Self::REFSEL)).unwrap()
    }

    pub fn set_reference(&mut self, reference: Reference) {
        self.0.set_field(Self::REFSEL, reference.into());
    }

    pub fn channel_count(&self) -> ChannelCount {
        ChannelCount::try_from(self.0.field(Self::CHCON)).unwrap()
    }

    pub fn set_channel_count(&mut self, count: ChannelCount) {
        self.0.set_field(Self::CHCON, count.into());
    }

    pub fn oscillator_power_down(&self) -> bool {
        self.0.bit(Self::OSCPD)
    }

    pub fn set_oscillator_power_down(&mut self, enabled: bool) {
        self.0.set_bit(Self::OSCPD, enabled);
    }

    pub fn adc_mode(&self) -> AdcMode {
        AdcMode::try_from(self.0.field(Self::ADC_MODE)).unwrap()
    }

    pub fn set_adc_mode(&mut self, mode: AdcMode) {
        self.0.set_field(Self::ADC_MODE, mode.into());
    }
}
