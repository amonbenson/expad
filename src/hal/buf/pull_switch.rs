use super::shift_register::ShiftRegisterChain;
use embassy_rp::spi;

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum PullSwitchChainError {
    Spi(spi::Error),
}

/// How one tap is driven: switched to the pull-up rail, to the pull-down rail, or to neither.
///
/// Every state closes at most one of the tap's two switches, so no value of this type can
/// short a pull-up resistor to a pull-down one through the same tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, defmt::Format)]
pub enum TriState {
    Low,
    High,
    #[default]
    HiZ,
}

/// Taps every shift register switches, one per contact of its jack.
pub const TAPS_PER_CHIP: usize = 4;

/// The shift register bits closing one tap's pull-up and pull-down switch.
struct TapSwitchBits {
    pull_up: u8,
    pull_down: u8,
}

/// Bit of each tap's two TMUX1511 switches in its 74HC595's output byte, indexed by tap.
/// Bit 7 is QH, bit 0 is QA, since the SPI shifts the most significant bit out first. The
/// pull-up switches sit on one TMUX1511, the pull-down switches on the other, and a high
/// select input closes the switch.
const TAP_SWITCH_BITS: [TapSwitchBits; TAPS_PER_CHIP] = [
    // Tap 0: QH -> pull-up SEL3, QF -> pull-down SEL3
    TapSwitchBits {
        pull_up: 7,
        pull_down: 5,
    },
    // Tap 1: QG -> pull-up SEL4, QE -> pull-down SEL4
    TapSwitchBits {
        pull_up: 6,
        pull_down: 4,
    },
    // Tap 2: QB -> pull-up SEL1, QD -> pull-down SEL1
    TapSwitchBits {
        pull_up: 1,
        pull_down: 3,
    },
    // Tap 3: QA -> pull-up SEL2, QC -> pull-down SEL2
    TapSwitchBits {
        pull_up: 0,
        pull_down: 2,
    },
];

pub type PullSwitchOutputState = [TriState; TAPS_PER_CHIP];

/// Pull-up and pull-down switches of `N` jacks, one 74HC595 per jack driving the select
/// inputs of two TMUX1511 quad switches: one connecting each tap to that jack's shared
/// pull-up resistor, the other to its shared pull-down resistor.
///
/// Chips are numbered from the microcontroller outwards, so chip 0 is the first one in the
/// shift register chain.
pub struct PullSwitchChain<'d, const N: usize> {
    sr_chain: ShiftRegisterChain<'d, N>,
    outputs: [PullSwitchOutputState; N],
}

impl<'d, const N: usize> PullSwitchChain<'d, N> {
    pub fn new(sr_chain: ShiftRegisterChain<'d, N>) -> Self {
        Self {
            sr_chain,
            outputs: [PullSwitchOutputState::default(); N],
        }
    }

    pub fn set_output(&mut self, chip: usize, tap: u8, state: TriState) {
        self.outputs[chip][tap as usize] = state;
    }

    pub fn get_output(&self, chip: usize, tap: u8) -> TriState {
        self.outputs[chip][tap as usize]
    }

    /// Applies every output state set since the last update, all at the same time.
    pub fn update(&mut self) -> Result<(), PullSwitchChainError> {
        let mut data = [0u8; N];

        // The first byte shifted out travels furthest down the chain, so it belongs to the
        // last chip.
        for (chip_data, outputs) in data.iter_mut().rev().zip(self.outputs.iter()) {
            *chip_data = encode(outputs);
        }

        self.sr_chain.write(data).map_err(PullSwitchChainError::Spi)
    }

    /// Opens every switch, leaving all taps floating.
    pub fn clear(&mut self) -> Result<(), PullSwitchChainError> {
        self.outputs = [PullSwitchOutputState::default(); N];
        self.update()
    }
}

fn encode(outputs: &PullSwitchOutputState) -> u8 {
    outputs
        .iter()
        .zip(TAP_SWITCH_BITS.iter())
        .fold(0u8, |byte, (state, bits)| match state {
            TriState::High => byte | 1 << bits.pull_up,
            TriState::Low => byte | 1 << bits.pull_down,
            TriState::HiZ => byte,
        })
}
