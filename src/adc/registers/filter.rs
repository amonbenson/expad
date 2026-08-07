use core::ops::RangeInclusive;

use super::{RegisterValue, Register};

pub const MODULATOR_FREQUENCY: u32 = 32_768;

pub const CHOPPING_DIVISOR: u32 = 3 * 8;
pub const CHOPPING_RANGE: RangeInclusive<u32> = 13..=255;

pub const NON_CHOPPING_DIVISOR: u32 = 8;
pub const NON_CHOPPING_RANGE: RangeInclusive<u32> = 3..=255;

pub type Filter = RegisterValue<0x03, 1>;

// impl Default for Filter {
//     fn default() -> Self {
//         Self(69)
//     }
// }

impl Filter {
    pub fn from_update_rate(update_rate: u32, chopping: bool) -> Self {
        // Inverse update rate formula from datasheet
        let divisor = if chopping { CHOPPING_DIVISOR } else { NON_CHOPPING_DIVISOR };
        let denominator = divisor.saturating_mul(update_rate.max(1));
        let value = MODULATOR_FREQUENCY / denominator;

        // Clamp to the valid range
        let range = if chopping { CHOPPING_RANGE } else { NON_CHOPPING_RANGE };
        Self(value.clamp(*range.start(), *range.end()))
    }

    pub fn update_rate(&self, chopping: bool) -> u32 {
        // Update rate formula from datasheet
        let divisor = if chopping { CHOPPING_DIVISOR } else { NON_CHOPPING_DIVISOR };
        MODULATOR_FREQUENCY / (divisor * self.bits())
    }

    pub fn best_speed(chopping: bool) -> Self {
        Self(if chopping { *CHOPPING_RANGE.start() } else { *NON_CHOPPING_RANGE.start() })
    }

    pub fn best_accuracy(chopping: bool) -> Self {
        Self(if chopping { *CHOPPING_RANGE.end() } else { *NON_CHOPPING_RANGE.end() })
    }
}
