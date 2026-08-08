use crate::sr::ShiftRegisterChain;
use embassy_rp::spi;

#[derive(Debug, Clone, Copy)]
pub enum QuadBufferChainError {
    Spi(spi::Error),
    IdMismatch {
        chip: u8,
        expected_id: u8,
        actual_id: u8,
    },
    InvalidChannel {
        chip: u8,
        channel: u8,
    },
    ContinuousMeasurementNotRunning,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TriState {
    Low = 0b10,
    High = 0b11,
    #[default]
    HiZ = 0b00,
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

    pub fn set_output(&mut self, chip: usize, channel: u8, state: TriState) {
        self.outputs[chip][channel as usize] = state;
    }

    pub fn get_output(&self, chip: usize, channel: u8) -> TriState {
        self.outputs[chip][channel as usize]
    }

    pub fn update(&mut self) -> Result<(), QuadBufferChainError> {
        let mut data = [0u8; N];

        // Generate shift register data from the output states.
        for chip in 0..N {
            for channel in 0..4 {
                let state = self.outputs[chip][channel];
                data[chip] |= (state as u8) << (channel * 2);
            }
        }

        self.sr_chain.write(data).map_err(|e| QuadBufferChainError::Spi(e))
    }

    pub fn clear(&mut self) -> Result<(), QuadBufferChainError> {
        self.sr_chain.clear().map_err(|e| QuadBufferChainError::Spi(e))
    }
}
