#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use expad::board::{self, ADC_CHIPS, Contact, JACKS};
use expad::hal::adc::{AdcChainConfig, Coding};
use expad::hal::buf::TriState;

use {defmt_rtt as _, panic_probe as _};

/// Inputs every AD7718 converts in the board's channel mode.
const ADC_CHANNELS: usize = board::ADC_CHANNEL_COUNT.count() as usize;

/// Bipolar, so readings around 0 V show their noise instead of clipping at zero.
const CODING: Coding = Coding::Bipolar;

/// State every jack's contacts are held in while capturing, indexed by [`Contact`]: all
/// `HiZ` to watch the inputs float, all `Low` or `High` to see the noise on a known voltage,
/// or one contact high and another low to reproduce a solver pair measurement on a pedal.
const DRIVE: [TriState; Contact::ALL.len()] = [TriState::HiZ; Contact::ALL.len()];

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
    let peripherals = embassy_rp::init(Default::default());

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(
        peripherals.SPI1,
        peripherals.PIN_14,
        peripherals.PIN_15,
        peripherals.PIN_11,
        peripherals.PIN_13,
    );
    for wiring in JACKS {
        for contact in Contact::ALL {
            switches.set_output(wiring.switch_chip, contact.tap(), DRIVE[contact as usize]);
        }
    }
    unwrap!(switches.update());

    info!("Initializing ADCs");
    let mut adcs = board::adcs(
        peripherals.SPI0,
        peripherals.PIN_18,
        peripherals.PIN_19,
        peripherals.PIN_16,
        peripherals.PIN_17,
        peripherals.PIN_20,
        peripherals.PIN_21,
        peripherals.PIN_22,
    );
    let adc_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE)
        // As in `expression_controller`, so the noise seen here is the noise the solver
        // works with.
        .with_update_rate(board::ADC_UPDATE_RATE)
        .with_coding(CODING);
    unwrap!(adcs.init(adc_config).await);

    info!("Taking individual measurements");
    for chip in 0..ADC_CHIPS {
        for channel in 0..ADC_CHANNELS as u8 {
            let voltage = unwrap!(adcs.measure_channel(chip, channel).await);
            info!("chip {} channel {}: {}V", chip, channel, voltage);
        }
    }

    info!("Starting continuous capture, contacts {}", DRIVE);
    let mut voltages = [[0.0; ADC_CHANNELS]; ADC_CHIPS];

    unwrap!(adcs.start_continuous_capture());
    loop {
        let measurement = unwrap!(adcs.wait_for_next_result().await);
        let chip = measurement.chip as usize;
        voltages[chip][measurement.channel as usize] = measurement.voltage;

        // Report each chip once per round through its inputs.
        if measurement.channel as usize == ADC_CHANNELS - 1 {
            info!("chip {}: {}V", chip, voltages[chip]);
        }
    }
}
