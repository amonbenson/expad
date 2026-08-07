use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::mode::ChannelCount;
use super::RegisterValue;

pub type Control = RegisterValue<0x02, 1>;

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum ChannelConfiguration {
    Single1 = 0b0000,
    Single2 = 0b0001,
    Single3 = 0b0010,
    Single4 = 0b0011,
    Single5 = 0b0100,
    Single6 = 0b0101,
    Single7 = 0b0110,
    Single8 = 0b0111,
    Diff1_2 = 0b1000,
    Diff3_4 = 0b1001,
    Diff5_6 = 0b1010,
    Diff7_8 = 0b1011,
    Diff9_10 = 0b1100,
    AInCom = 0b1101,
    RefIn_Single9 = 0b1110,
    Open_Single10 = 0b1111,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    AIn(u8),
    AInCom,
    RefInPositive,
    RefInNegative,
    Open,
}

impl ChannelConfiguration {
    pub const fn single(input: u8) -> Option<Self> {
        match input {
            1 => Some(Self::Single1),
            2 => Some(Self::Single2),
            3 => Some(Self::Single3),
            4 => Some(Self::Single4),
            5 => Some(Self::Single5),
            6 => Some(Self::Single6),
            7 => Some(Self::Single7),
            8 => Some(Self::Single8),
            _ => None,
        }
    }

    pub const fn differential(positive_input: u8, negative_input: u8) -> Option<Self> {
        match (positive_input, negative_input) {
            (1, 2) => Some(Self::Diff1_2),
            (3, 4) => Some(Self::Diff3_4),
            (5, 6) => Some(Self::Diff5_6),
            (7, 8) => Some(Self::Diff7_8),
            (9, 10) => Some(Self::Diff9_10),
            _ => None,
        }
    }

    pub fn inputs(self, channel_count: ChannelCount) -> (Input, Input) {
        use ChannelCount::{Eight, Ten};
        use Input::{AIn, AInCom, Open, RefInPositive, RefInNegative};

        match (self, channel_count) {
            (Self::Single1, _) => (AIn(1), AInCom),
            (Self::Single2, _) => (AIn(2), AInCom),
            (Self::Single3, _) => (AIn(3), AInCom),
            (Self::Single4, _) => (AIn(4), AInCom),
            (Self::Single5, _) => (AIn(5), AInCom),
            (Self::Single6, _) => (AIn(6), AInCom),
            (Self::Single7, _) => (AIn(7), AInCom),
            (Self::Single8, _) => (AIn(8), AInCom),
            (Self::Diff1_2, _) => (AIn(1), AIn(2)),
            (Self::Diff3_4, _) => (AIn(3), AIn(4)),
            (Self::Diff5_6, _) => (AIn(5), AIn(6)),
            (Self::Diff7_8, _) => (AIn(7), AIn(8)),
            (Self::Diff9_10, Eight) => (AIn(2), AIn(2)),
            (Self::Diff9_10, Ten) => (AIn(9), AIn(10)),
            (Self::AInCom, _) => (AInCom, AInCom),
            (Self::RefIn_Single9, Eight) => (RefInPositive, RefInNegative),
            (Self::RefIn_Single9, Ten) => (AIn(9), AInCom),
            (Self::Open_Single10, Eight) => (Open, Open),
            (Self::Open_Single10, Ten) => (AIn(10), AInCom),
        }
    }

    pub fn calibration_register(self, channel_count: ChannelCount) -> u8 {
        use ChannelCount::{Eight, Ten};

        match (self, channel_count) {
            (Self::Single1, _) => 1,
            (Self::Single2, _) => 2,
            (Self::Single3, _) => 3,
            (Self::Single4, _) => 4,
            (Self::Single5, Eight) => 1,
            (Self::Single5, Ten) => 5,
            (Self::Single6, Eight) => 2,
            (Self::Single6, Ten) => 1,
            (Self::Single7, Eight) => 3,
            (Self::Single7, Ten) => 2,
            (Self::Single8, Eight) => 4,
            (Self::Single8, Ten) => 3,
            (Self::Diff1_2, _) => 1,
            (Self::Diff3_4, _) => 2,
            (Self::Diff5_6, _) => 3,
            (Self::Diff7_8, _) => 4,
            (Self::Diff9_10, Eight) => 1,
            (Self::Diff9_10, Ten) => 5,
            (Self::AInCom, _) => 1,
            (Self::RefIn_Single9, Eight) => 1,
            (Self::RefIn_Single9, Ten) => 4,
            (Self::Open_Single10, Eight) => 1,
            (Self::Open_Single10, Ten) => 5,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
pub enum Coding {
    Bipolar = 0b0,
    Unipolar = 0b1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum Range {
    Range20mV = 0b000,
    Range40mV = 0b001,
    Range80mV = 0b010,
    Range160mV = 0b011,
    Range320mV = 0b100,
    Range640mV = 0b101,
    Range1_28V = 0b110,
    Range2_56V = 0b111,
}

impl Control {
    pub fn channel_configuration(&self) -> ChannelConfiguration {
        self.field(4, 4)
    }

    pub fn set_channel_configuration(&mut self, value: ChannelConfiguration) {
        self.set_field(4, 4, value);
    }

    pub fn coding(&self) -> Coding {
        self.field(3, 1)
    }

    pub fn set_coding(&mut self, value: Coding) {
        self.set_field(3, 1, value);
    }

    pub fn range(&self) -> Range {
        self.field(0, 3)
    }

    pub fn set_range(&mut self, value: Range) {
        self.set_field(0, 3, value);
    }
}
