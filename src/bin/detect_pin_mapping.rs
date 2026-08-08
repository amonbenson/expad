#![no_std]
#![no_main]

use defmt::{info, warn};
use embassy_executor::Spawner;
use expad::hal::adc::{AdcChain, AdcChainConfig, ChannelCount};
use expad::hal::buf::{QuadBufferChain, ShiftRegisterChain, TriState};

use {defmt_rtt as _, panic_probe as _};

const CHANNELS: usize = 1;
const BUF_CHANNELS: u8 = 4;
const ADC_CHANNELS: u8 = 10;

/// A match's delta below this fraction of the best delta seen across all buffer
/// outputs is flagged as low-confidence rather than reported as a solid match.
const CONFIDENCE_THRESHOLD: f32 = 0.5;

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Pin Mapping Detector"),
    embassy_rp::binary_info::rp_program_description!(
        c"Detects which ADC input each buffer output is wired to"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[derive(Clone, Copy, defmt::Format)]
struct Mapping {
    adc_channel: u8,
    delta: f32,
    low: f32,
    high: f32,
}

async fn measure_all(adcs: &mut AdcChain<'_, CHANNELS>) -> [f32; ADC_CHANNELS as usize] {
    let mut voltages = [0.0; ADC_CHANNELS as usize];
    for (channel, voltage) in voltages.iter_mut().enumerate() {
        *voltage = adcs.measure_channel(0, channel as u8).await.unwrap();
    }
    voltages
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing tristate buffers");
    let sr = ShiftRegisterChain::<CHANNELS>::new(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    let mut buffers = QuadBufferChain::new(sr);
    buffers.clear().unwrap();

    info!("Initializing ADCs");
    let mut adcs =
        AdcChain::<CHANNELS>::new(p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, [p.PIN_17], [p.PIN_21]);
    adcs.init(AdcChainConfig::default().with_channel_count(ChannelCount::Ten))
        .await
        .unwrap();

    info!("Detecting buffer-to-ADC pin mapping");
    let mut mappings = [None::<Mapping>; BUF_CHANNELS as usize];

    for buf_channel in 0..BUF_CHANNELS {
        buffers.set_output(0, buf_channel, TriState::Low);
        buffers.update().unwrap();
        let low = measure_all(&mut adcs).await;

        buffers.set_output(0, buf_channel, TriState::High);
        buffers.update().unwrap();
        let high = measure_all(&mut adcs).await;

        buffers.set_output(0, buf_channel, TriState::HiZ);
        buffers.update().unwrap();

        let (adc_channel, delta) = (0..ADC_CHANNELS)
            .map(|c| (c, (high[c as usize] - low[c as usize]).abs()))
            .fold(
                (0u8, 0.0f32),
                |best, cur| if cur.1 > best.1 { cur } else { best },
            );

        mappings[buf_channel as usize] = Some(Mapping {
            adc_channel,
            delta,
            low: low[adc_channel as usize],
            high: high[adc_channel as usize],
        });
    }

    let max_delta = mappings
        .iter()
        .map(|mapping| mapping.unwrap().delta)
        .fold(0.0f32, f32::max);

    info!("Buffer output -> ADC channel mapping:");
    for (buf_channel, mapping) in mappings.iter().enumerate() {
        let mapping = mapping.unwrap();
        if mapping.delta < max_delta * CONFIDENCE_THRESHOLD {
            warn!(
                "buffer channel {} -> adc channel {} is a WEAK match (changed by only {}V, {}V -> {}V) - check the hardware connection",
                buf_channel, mapping.adc_channel, mapping.delta, mapping.low, mapping.high
            );
        } else {
            info!(
                "buffer channel {} -> adc channel {} (changed by {}V, {}V -> {}V)",
                buf_channel, mapping.adc_channel, mapping.delta, mapping.low, mapping.high
            );
        }
    }
}
