use crate::sr::ShiftRegisterChain;
use embassy_rp::spi::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum TriState {
    Low,
    High,
    #[default]
    HiZ,
}

impl TriState {
    pub fn to_bits(&self) -> u8 {
        match self {
            TriState::Low => 0b10,
            TriState::High => 0b11,
            TriState::HiZ => 0b00,
        }
    }
}

pub type QuadBufferOutputState = [TriState; 4];

pub struct QuadBufferChain<'d, const N: usize>
{
    sr_chain: ShiftRegisterChain<'d, N>,
    outputs: [QuadBufferOutputState; N],
}

impl<'d, const N: usize> QuadBufferChain<'d, N> {
    pub fn new(
        sr_chain: ShiftRegisterChain<'d, N>,
    ) -> Self {
        Self {
            sr_chain,
            outputs: [QuadBufferOutputState::default(); N],
        }
    }

    pub fn set_output(&mut self, chip: usize, channel: usize, state: TriState) {
        self.outputs[chip][channel] = state;
    }

    pub fn get_output(&self, chip: usize, channel: usize) -> TriState {
        self.outputs[chip][channel]
    }

    pub fn update(&mut self) -> Result<(), Error> {
        let mut data = [0u8; N];

        // Generate shift register data from the output states.
        for chip in 0..N {
            for channel in 0..4 {
                let state = self.outputs[chip][channel];
                data[chip] |= state.to_bits() << (channel * 2);
            }
        }

        self.sr_chain.write(data)
    }

    pub fn clear(&mut self) -> Result<(), Error> {
        self.sr_chain.clear()
    }
}
