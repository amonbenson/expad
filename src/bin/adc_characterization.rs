#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use expad::board::{self, Contact, JACKS};
use expad::hal::adc::{AdcChain, AdcChainConfig, Coding};
use expad::hal::buf::TriState;

use {defmt_rtt as _, panic_probe as _};

/// Jack the potentiometer pedal is plugged into, and how its contacts are driven: the two
/// track ends to the rails and the wiper left floating, as tracking it would.
const JACK: usize = 0;
const HIGH_CONTACT: Contact = Contact::Ring;
const LOW_CONTACT: Contact = Contact::Sleeve;
const WIPER_CONTACT: Contact = Contact::Tip;

/// Update rates to characterize, one per filter word from the fastest (3) to the
/// controller's current one (13). Each lands on its filter word after the driver's
/// rounding down of 4096 / rate.
const UPDATE_RATES: [u32; 5] = [1365, 1024, 819, 682, 315];

/// Readings averaged for every rail and self-test value.
const AVERAGED_READINGS: usize = 16;

/// Rounds of tip, ring and sleeve single conversions, as a solver or a multi-jack tracker
/// switching channels would take them.
const SWITCHING_ROUNDS: usize = 150;

/// Consecutive results taken without leaving the wiper channel.
const CONTINUOUS_SAMPLES: usize = 1024;

/// Window of the moving average that notches out mains hum and its harmonics.
const MAINS_FREQUENCY: f32 = 50.0;

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"ADC Characterization"),
    embassy_rp::binary_info::rp_program_description!(
        c"Measures noise, timing and calibration of the AD7718 at every filter setting on a pedal"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

/// `core` has no floating point square root; Newton's method converges in a handful of
/// steps for the magnitudes seen here.
fn square_root(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut root = if value > 1.0 { value } else { 1.0 };
    for _ in 0..40 {
        root = 0.5 * (root + value / root);
    }
    root
}

/// Cosine by its Taylor series, accurate for the angles below π this tool needs.
fn cosine(angle: f32) -> f32 {
    let squared = angle * angle;
    let mut term = 1.0;
    let mut sum = 1.0;
    for order in 1..12 {
        term *= -squared / ((2 * order - 1) * (2 * order)) as f32;
        sum += term;
    }
    sum
}

#[derive(defmt::Format)]
struct Statistics {
    /// Mean, in V.
    mean: f32,
    /// Standard deviation around the mean, in µV.
    deviation: f32,
    /// Standard deviation of successive differences over √2, in µV: the sample-to-sample
    /// noise, unaffected by slow drift.
    white: f32,
    /// Smallest nonzero step between two readings, in µV: the output's resolution.
    step: f32,
}

fn statistics(samples: &[f32]) -> Statistics {
    let count = samples.len() as f32;
    let mean = samples.iter().sum::<f32>() / count;
    let variance = samples.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / count;

    let differences = samples.windows(2).map(|pair| pair[1] - pair[0]);
    let difference_variance = differences.clone().map(|d| d * d).sum::<f32>() / (count - 1.0);
    let step = differences
        .map(f32::abs)
        .filter(|&d| d > 0.0)
        .fold(f32::INFINITY, f32::min);

    Statistics {
        mean,
        deviation: square_root(variance) * 1e6,
        white: square_root(difference_variance / 2.0) * 1e6,
        step: step * 1e6,
    }
}

/// Standard deviation of the means of consecutive blocks of `size`, in µV - the noise left
/// after averaging that many results.
fn block_average_deviation(samples: &[f32], size: usize) -> f32 {
    let mut averages = [0.0; CONTINUOUS_SAMPLES];
    let count = samples.len() / size;
    for (average, block) in averages.iter_mut().zip(samples.chunks_exact(size)) {
        *average = block.iter().sum::<f32>() / size as f32;
    }
    statistics(&averages[..count]).deviation
}

/// Standard deviation of a moving average over `window` results, in µV.
fn moving_average_deviation(samples: &[f32], window: usize) -> f32 {
    let mut averages = [0.0; CONTINUOUS_SAMPLES];
    let count = samples.len() - window + 1;
    for (average, run) in averages.iter_mut().zip(samples.windows(window)) {
        *average = run.iter().sum::<f32>() / window as f32;
    }
    statistics(&averages[..count]).deviation
}

/// Amplitude of the `frequency` component of evenly spaced `samples`, in µV (Goertzel).
fn tone_amplitude(samples: &[f32], sample_rate: f32, frequency: f32) -> f32 {
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let coefficient = 2.0 * cosine(2.0 * core::f32::consts::PI * frequency / sample_rate);
    let (mut previous, mut before_previous) = (0.0f32, 0.0f32);
    for sample in samples {
        let current = sample - mean + coefficient * previous - before_previous;
        before_previous = previous;
        previous = current;
    }
    let power = previous * previous + before_previous * before_previous
        - coefficient * previous * before_previous;
    2.0 * square_root(power.max(0.0)) / samples.len() as f32 * 1e6
}

async fn average(adcs: &mut AdcChain<'_, { board::ADC_CHIPS }>, chip: usize, channel: u8) -> f32 {
    let mut sum = 0.0;
    for _ in 0..AVERAGED_READINGS {
        sum += unwrap!(adcs.measure_channel(chip, channel).await);
    }
    sum / AVERAGED_READINGS as f32
}

/// Ground and reference inputs plus both driven track ends, averaged: the values offset
/// and gain errors show up in.
async fn report_levels(adcs: &mut AdcChain<'_, { board::ADC_CHIPS }>, label: &str) {
    let wiring = JACKS[JACK];
    let chip = wiring.adc_chip;
    let ground = average(adcs, chip, board::ADC_GROUND_CHANNEL).await;
    let reference = average(adcs, chip, board::ADC_REFERENCE_CHANNEL).await;
    let high = average(adcs, chip, wiring.adc_channel(HIGH_CONTACT)).await;
    let low = average(adcs, chip, wiring.adc_channel(LOW_CONTACT)).await;
    info!(
        "  {}: ground {}V, reference {}V, high end {}V, low end {}V",
        label, ground, reference, high, low
    );
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let wiring = JACKS[JACK];
    let chip = wiring.adc_chip;
    let wiper_channel = wiring.adc_channel(WIPER_CONTACT);
    let high_channel = wiring.adc_channel(HIGH_CONTACT);
    let low_channel = wiring.adc_channel(LOW_CONTACT);

    let mut switches = board::pull_switches(p.SPI1, p.PIN_14, p.PIN_15, p.PIN_11, p.PIN_13);
    switches.set_output(wiring.switch_chip, HIGH_CONTACT.tap(), TriState::High);
    switches.set_output(wiring.switch_chip, LOW_CONTACT.tap(), TriState::Low);
    unwrap!(switches.update());

    let mut adcs = board::adcs(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.PIN_17, p.PIN_20, p.PIN_21, p.PIN_22,
    );
    let base_config = AdcChainConfig::default()
        .with_channel_count(board::ADC_CHANNEL_COUNT)
        .with_reference_voltage(board::REFERENCE_VOLTAGE)
        .with_coding(Coding::Bipolar);

    info!(
        "Jack {}: {} high, {} low, {} as wiper - keep the pedal still",
        JACK, HIGH_CONTACT, LOW_CONTACT, WIPER_CONTACT
    );
    Timer::after_millis(100).await;

    let mut wiper = [0.0f32; SWITCHING_ROUNDS];
    let mut positions = [0.0f32; SWITCHING_ROUNDS];
    let mut continuous = [0.0f32; CONTINUOUS_SAMPLES];

    for update_rate in UPDATE_RATES {
        unwrap!(adcs.init(base_config.with_update_rate(update_rate)).await);
        info!("=== {} Hz (calibrated here)", adcs.update_rate());
        report_levels(&mut adcs, "levels").await;

        // Switching channel for every reading, as the solver does.
        let started = Instant::now();
        for round in 0..SWITCHING_ROUNDS {
            let tip = unwrap!(adcs.measure_channel(chip, wiper_channel).await);
            let high = unwrap!(adcs.measure_channel(chip, high_channel).await);
            let low = unwrap!(adcs.measure_channel(chip, low_channel).await);
            wiper[round] = tip;
            positions[round] = (tip - low) / (high - low);
        }
        let reading_micros = started.elapsed().as_micros() / (3 * SWITCHING_ROUNDS) as u64;
        let position = statistics(&positions);
        info!(
            "  switching: {} us per reading, wiper {}, position {} (deviation {} ppm)",
            reading_micros,
            statistics(&wiper),
            position.mean,
            position.deviation
        );

        // Staying on the wiper channel, as a single tracked pedal per ADC can.
        let started = Instant::now();
        unwrap!(
            adcs.measure_continuous(chip, wiper_channel, &mut continuous)
                .await
        );
        let period_micros = started.elapsed().as_micros() as f32 / CONTINUOUS_SAMPLES as f32;
        let sample_rate = 1e6 / period_micros;
        let mains_window = (sample_rate / MAINS_FREQUENCY + 0.5) as usize;
        info!(
            "  continuous: {} us per result, wiper {}",
            period_micros,
            statistics(&continuous)
        );
        info!(
            "  continuous averaged: 4 -> {} uV, 8 -> {} uV, {} (20 ms) -> {} uV; 50 Hz {} uV, 100 Hz {} uV, 150 Hz {} uV",
            block_average_deviation(&continuous, 4),
            block_average_deviation(&continuous, 8),
            mains_window,
            moving_average_deviation(&continuous, mains_window),
            tone_amplitude(&continuous, sample_rate, MAINS_FREQUENCY),
            tone_amplitude(&continuous, sample_rate, 2.0 * MAINS_FREQUENCY),
            tone_amplitude(&continuous, sample_rate, 3.0 * MAINS_FREQUENCY)
        );

        unwrap!(
            adcs.measure_continuous(chip, high_channel, &mut continuous[..256])
                .await
        );
        info!(
            "  continuous high end (1 kΩ source): {}",
            statistics(&continuous[..256])
        );
    }

    // Whether a calibration taken at one filter setting still holds at another.
    let slowest = UPDATE_RATES[UPDATE_RATES.len() - 1];
    let fastest = UPDATE_RATES[0];
    unwrap!(adcs.set_update_rate(fastest));
    info!(
        "=== {} Hz without recalibrating (calibrated at {} Hz)",
        adcs.update_rate(),
        slowest
    );
    report_levels(&mut adcs, "levels").await;

    unwrap!(switches.clear());
    info!("Done");
}
