#![no_std]
#![no_main]

use defmt::{error, info, unwrap, warn};
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use expad::board::{self, ADC_CHIPS, Contact, JACKS};
use expad::hal::adc::{AdcChain, AdcChainConfig};
use expad::hal::buf::TriState;

use {defmt_rtt as _, panic_probe as _};

/// Inputs every AD7718 converts in ten-channel mode.
const ADC_CHANNELS: usize = 10;

/// How long a tap is left to settle after its switches change. The slowest node is the ADC
/// input filter charging through the pull resistor, 11 kΩ x 10 nF = 110 µs.
const SETTLE_DELAY: Duration = Duration::from_millis(5);

/// Swing between pulled up and pulled down above which an input counts as following a tap.
/// A directly switched tap swings the whole reference, so anything connected to it through
/// a pedal still clears this comfortably, while floating inputs merely drift.
const RESPONSE_THRESHOLD: f32 = 1.0;

/// Furthest a pulled tap, or a self-test input, may read from its rail.
const RAIL_TOLERANCE: f32 = 0.05;

type Readings = [[f32; ADC_CHANNELS]; ADC_CHIPS];

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Pin Mapping Detector"),
    embassy_rp::binary_info::rp_program_description!(
        c"Checks every jack contact's pull switches and ADC input against the board table"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

async fn measure_all(adcs: &mut AdcChain<'_, ADC_CHIPS>) -> Readings {
    let mut readings = [[0.0; ADC_CHANNELS]; ADC_CHIPS];
    for channel in 0..ADC_CHANNELS {
        let voltages = unwrap!(
            adcs.measure_parallel([Some(channel as u8); ADC_CHIPS])
                .await
        );
        for (chip_readings, voltage) in readings.iter_mut().zip(voltages) {
            chip_readings[channel] = unwrap!(voltage);
        }
    }
    readings
}

/// The jack contact the board table puts on an ADC input, if any.
fn wired_to(chip: usize, channel: u8) -> Option<(usize, Contact)> {
    JACKS.iter().enumerate().find_map(|(jack, wiring)| {
        let contact = Contact::ALL
            .into_iter()
            .find(|&contact| wiring.adc_channel(contact) == channel)?;
        (wiring.adc_chip == chip).then_some((jack, contact))
    })
}

/// Whether two contacts are expected to follow each other: the tip and its normalling
/// contact touch while no plug is inserted.
fn normalled(first: Contact, second: Contact) -> bool {
    matches!(
        (first, second),
        (Contact::Tip, Contact::TipSwitch) | (Contact::TipSwitch, Contact::Tip)
    )
}

fn near(voltage: f32, rail: f32) -> bool {
    (voltage - rail).abs() <= RAIL_TOLERANCE
}

/// Reads every AD7718's grounded and reference-tied input, which checks the converters,
/// their reference and the voltage scaling before anything is switched. Returns how many
/// chips failed.
async fn self_test_adcs(adcs: &mut AdcChain<'_, ADC_CHIPS>) -> u32 {
    let mut failures = 0;

    for chip in 0..ADC_CHIPS {
        let ground = unwrap!(adcs.measure_channel(chip, board::ADC_GROUND_CHANNEL).await);
        let reference = unwrap!(
            adcs.measure_channel(chip, board::ADC_REFERENCE_CHANNEL)
                .await
        );

        if near(ground, 0.0) && near(reference, board::REFERENCE_VOLTAGE) {
            info!(
                "ADC {}: ground input {}V, reference input {}V - OK",
                chip, ground, reference
            );
        } else {
            error!(
                "ADC {}: ground input {}V (expected 0V), reference input {}V (expected {}V)",
                chip,
                ground,
                reference,
                board::REFERENCE_VOLTAGE
            );
            failures += 1;
        }
    }

    failures
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    unwrap!(switches.clear());

    info!("Initializing ADCs");
    let mut adcs = board::adcs(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_20, p.PIN_21, p.PIN_22,
    );
    let adc_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE);
    unwrap!(adcs.init(adc_config).await);
    info!("ADC full scale: {}V", adcs.full_scale_voltage());

    let mut failures = self_test_adcs(&mut adcs).await;

    info!("Switching every contact to both rails - unplug all jacks for a clean result");
    let started = Instant::now();
    for (jack, wiring) in JACKS.iter().enumerate() {
        for contact in Contact::ALL {
            let tap = contact.tap();

            switches.set_output(wiring.switch_chip, tap, TriState::High);
            unwrap!(switches.update());
            Timer::after(SETTLE_DELAY).await;
            let high = measure_all(&mut adcs).await;

            switches.set_output(wiring.switch_chip, tap, TriState::Low);
            unwrap!(switches.update());
            Timer::after(SETTLE_DELAY).await;
            let low = measure_all(&mut adcs).await;

            switches.set_output(wiring.switch_chip, tap, TriState::HiZ);
            unwrap!(switches.update());

            let expected_chip = wiring.adc_chip;
            let expected_channel = wiring.adc_channel(contact);
            let expected_high = high[expected_chip][expected_channel as usize];
            let expected_low = low[expected_chip][expected_channel as usize];

            if near(expected_high, board::REFERENCE_VOLTAGE) && near(expected_low, 0.0) {
                info!(
                    "Jack {} {} (switch chip {} tap {}) -> ADC {} channel {}: {}V / {}V - OK",
                    jack,
                    contact,
                    wiring.switch_chip,
                    tap,
                    expected_chip,
                    expected_channel,
                    expected_high,
                    expected_low
                );
            } else {
                error!(
                    "Jack {} {} (switch chip {} tap {}) -> ADC {} channel {}: {}V pulled up (expected {}V), {}V pulled down (expected 0V)",
                    jack,
                    contact,
                    wiring.switch_chip,
                    tap,
                    expected_chip,
                    expected_channel,
                    expected_high,
                    board::REFERENCE_VOLTAGE,
                    expected_low
                );
                failures += 1;
            }

            // Every other input following this tap is either the normalled tip contact or
            // a wiring fault - or a pedal plugged into the jack.
            for chip in 0..ADC_CHIPS {
                for channel in 0..ADC_CHANNELS as u8 {
                    let swing = high[chip][channel as usize] - low[chip][channel as usize];
                    let is_expected = chip == expected_chip && channel == expected_channel;
                    if is_expected || swing.abs() < RESPONSE_THRESHOLD {
                        continue;
                    }

                    match wired_to(chip, channel) {
                        Some((other_jack, other_contact))
                            if other_jack == jack && normalled(contact, other_contact) =>
                        {
                            info!(
                                "    also follows on {} (ADC {} channel {}) - normalled, no plug inserted",
                                other_contact, chip, channel
                            );
                        }
                        Some((other_jack, other_contact)) => {
                            warn!(
                                "    also follows on jack {} {} (ADC {} channel {}, swing {}V) - a pedal plugged in, or a short",
                                other_jack, other_contact, chip, channel, swing
                            );
                        }
                        None => {
                            warn!(
                                "    also follows on unassigned ADC {} channel {} (swing {}V)",
                                chip, channel, swing
                            );
                        }
                    }
                }
            }
        }
    }

    unwrap!(switches.clear());
    info!(
        "Checked every contact in {}ms, both ADCs converting in parallel",
        started.elapsed().as_millis()
    );

    if failures == 0 {
        info!("Every contact matches the board table");
    } else {
        error!(
            "{} check(s) failed. Every tap near {}V in both states means the switch select polarity is inverted",
            failures,
            board::REFERENCE_VOLTAGE / 2.0
        );
    }
}
