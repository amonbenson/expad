#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use expad::board::{self, ADC_CHIPS};
use expad::hal::adc::AdcChainConfig;

use {defmt_rtt as _, panic_probe as _};

/// Inputs every AD7718 converts in ten-channel mode.
const ADC_CHANNELS: usize = 10;

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Capture"),
    embassy_rp::binary_info::rp_program_description!(
        c"Continuously captures every ADC input with all pull switches open"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    switches.clear().unwrap();

    info!("Initializing ADCs");
    let mut adcs = board::adcs(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_20, p.PIN_21, p.PIN_22,
    );
    let adc_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE);
    adcs.init(adc_config).await.unwrap();

    info!("Taking individual measurements");
    for chip in 0..ADC_CHIPS {
        for channel in 0..ADC_CHANNELS as u8 {
            let voltage = adcs.measure_channel(chip, channel).await.unwrap();
            info!("chip {} channel {}: {}V", chip, channel, voltage);
        }
    }

    info!("Starting continuous capture");
    let mut voltages = [[0.0; ADC_CHANNELS]; ADC_CHIPS];

    adcs.start_continuous_capture().unwrap();
    loop {
        let measurement = adcs.wait_for_next_result().await.unwrap();
        let chip = measurement.chip as usize;
        voltages[chip][measurement.channel as usize] = measurement.voltage;

        // Report each chip once per round through its inputs.
        if measurement.channel as usize == ADC_CHANNELS - 1 {
            info!("chip {}: {}V", chip, voltages[chip]);
        }
    }
}
