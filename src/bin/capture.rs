#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use expad::board::{self, ADC_CHIPS, Contact, JACKS};
use expad::hal::adc::{AdcChainConfig, Coding};
use expad::hal::buf::TriState;

use {defmt_rtt as _, panic_probe as _};

/// Inputs every AD7718 converts in ten-channel mode.
const ADC_CHANNELS: usize = 10;

/// Conversions per second, as in `expression_controller`, so the noise seen here is the noise
/// the solver works with.
const UPDATE_RATE: u32 = 819;

/// Bipolar, so readings around 0 V show their noise instead of clipping at zero.
const CODING: Coding = Coding::Bipolar;

/// State every jack's contacts are held in while capturing, indexed by [`Contact`]: all
/// `HiZ` to watch the inputs float, all `Low` or `High` to see the noise on a known voltage,
/// or one contact high and another low to reproduce a solver pair measurement on a pedal.
const DRIVE: [TriState; 4] = [TriState::HiZ; 4];

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Capture"),
    embassy_rp::binary_info::rp_program_description!(
        c"Continuously captures every ADC input with every jack contact held in one state"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    for wiring in JACKS {
        for contact in Contact::ALL {
            switches.set_output(wiring.switch_chip, contact.tap(), DRIVE[contact as usize]);
        }
    }
    switches.update().unwrap();

    info!("Initializing ADCs");
    let mut adcs = board::adcs(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_20, p.PIN_21, p.PIN_22,
    );
    let adc_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE)
        .with_update_rate(UPDATE_RATE)
        .with_coding(CODING);
    adcs.init(adc_config).await.unwrap();

    info!("Taking individual measurements");
    for chip in 0..ADC_CHIPS {
        for channel in 0..ADC_CHANNELS as u8 {
            let voltage = adcs.measure_channel(chip, channel).await.unwrap();
            info!("chip {} channel {}: {}V", chip, channel, voltage);
        }
    }

    info!("Starting continuous capture, contacts {}", DRIVE);
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
