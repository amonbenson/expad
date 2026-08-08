#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use hal::adc::{AdcChain, AdcChainConfig};
use hal::buf::{QuadBufferChain, ShiftRegisterChain};

use {defmt_rtt as _, panic_probe as _};

// `hal` and `topology` expose a broader register/API surface than `main` currently
// drives; see AGENTS.md's Extensibility Hooks for the parts still awaiting callers.
#[allow(dead_code)]
mod hal;
#[allow(dead_code)]
mod topology;

const CHANNELS: usize = 1;

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

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
    adcs.init(AdcChainConfig::default()).unwrap();

    info!("Taking individual measurements");
    for channel in 0..3u8 {
        let value = adcs.measure_channel(0, channel).await.unwrap();
        info!("chip 0 channel {}: {}", channel, value);
    }

    info!("Starting continuous capture");
    let mut measurements = [0.0; 8];

    adcs.start_continuous_capture().unwrap();
    loop {
        let measurement = adcs.wait_for_next_result().await.unwrap();
        measurements[measurement.channel as usize] = measurement.value as f32 / 0xFFFFFF as f32;
        info!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            measurements[0],
            measurements[1],
            measurements[2],
            measurements[3],
            measurements[4],
            measurements[5],
            measurements[6],
            measurements[7]
        );
    }
}
