#![no_std]
#![no_main]

use defmt::{debug, info, unwrap, warn};
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0, PIO1, USB};
use embassy_rp::{bind_interrupts, dma, pio, usb};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Ticker};
use expad::board::{self, JACKS};
use expad::hal::adc::AdcChainConfig;
use expad::hal::led::RGB8;
use expad::hal::usb::{
    CableNumber, Channel as MidiChannel, ControlFunction, FromClamped, Message, U7, UsbMidi,
    UsbMidiConfig,
};
use expad::hal::wifi::{AccessPointConfig, AccessPointPeripherals, start_access_point};
use expad::topology::solver::{
    ArmConfig, ArmRails, ResistanceSolver, ResistanceSolverConfig, SolveOutcome,
};
use expad::topology::{ARM_COUNT, ArmResistances, SolverConfig};
use expad::web::{
    ArmPull, INTERFACE, JACK_COUNT, JackSettings, JackStatus, Status, spawn_web_server,
};

use {defmt_rtt as _, panic_probe as _};

/// ADC conversions per second. The fastest rate leaves ~11 noise-free bits, too coarse to see
/// the few millivolts a high-value potentiometer drops across its pull resistors; 315 Hz is
/// four times quieter at about 10 ms per reading.
const ADC_UPDATE_RATE: u32 = 315;

/// Standard deviation of a single reading at [`ADC_UPDATE_RATE`], measured on the breadboard.
/// Every tolerance the solver applies is derived from it.
const ADC_VOLTAGE_NOISE: f32 = 0.001;

/// Colors the jacks are shown in, matching `JACK_COLORS` in web/src/theme.ts.
const JACK_COLORS: [RGB8; JACK_COUNT] = [
    RGB8 {
        r: 0xFF,
        g: 0x7E,
        b: 0x7E,
    },
    RGB8 {
        r: 0xFF,
        g: 0xA2,
        b: 0x59,
    },
    RGB8 {
        r: 0xFF,
        g: 0xCB,
        b: 0x56,
    },
    RGB8 {
        r: 0xFF,
        g: 0xED,
        b: 0xB9,
    },
];

/// Lower end of the brightness a connected jack's LED is scaled across, so a pedal at rest
/// still reads as connected rather than as an unplugged jack.
const CONNECTED_LED_FLOOR: f32 = 0.1;

/// Shortest time one full sweep of every jack may take. Measuring is bounded by the ADC's
/// conversion time in practice, but an empty mapping table would otherwise spin the loop
/// without ever yielding to the network stack.
const MINIMUM_SWEEP_INTERVAL: Duration = Duration::from_millis(20);

/// Sweeps between re-measuring the rail voltages. They are properties of the board rather
/// than of what is plugged in, so they only have to keep up with thermal drift.
const RAIL_REFRESH_SWEEPS: u32 = 500;

/// Sweeps between logging how long one takes, which is what the detection rate comes out
/// of. Sweeps that remeasure the rails are reported separately, since they are not typical.
const SWEEP_REPORT_SWEEPS: u32 = 100;

/// Highest value of a MIDI control change.
const MIDI_MAX_VALUE: u8 = 127;

/// How far the wiper has to move before a new control change is sent, as a fraction of
/// full travel. Slightly wider than one control change step, so a position resting between
/// two steps does not alternate between them.
const MIDI_HYSTERESIS: f32 = 0.6 / MIDI_MAX_VALUE as f32;

/// Control changes waiting to be sent. The measurement loop must never block on USB, so it
/// hands finished messages over and drops them if the host is not keeping up.
const MIDI_QUEUE_DEPTH: usize = 8;
static MIDI_UPDATES: Channel<CriticalSectionRawMutex, MidiUpdate, MIDI_QUEUE_DEPTH> =
    Channel::new();

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Expression Controller"),
    embassy_rp::binary_info::rp_program_description!(
        c"Continuously detects the resistor topology behind every jack, serves the web interface over WiFi and sends each pedal's position as a MIDI control change"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

/// A control change the measurement loop has decided to send.
#[derive(Debug, Clone, Copy, defmt::Format)]
struct MidiUpdate {
    channel: u8,
    controller: u8,
    value: u8,
}

/// What the last control change sent for a jack was, and the wiper position it was
/// quantized from.
#[derive(Debug, Clone, Copy)]
struct SentControlChange {
    position: f32,
    value: u8,
}

/// Quantizes a wiper position to a control change value, inverted for jacks whose pedal is
/// wired the other way round.
fn control_value(position: f32, settings: &JackSettings) -> u8 {
    let position = if settings.inverted {
        1.0 - position
    } else {
        position
    };

    (position.clamp(0.0, 1.0) * MIDI_MAX_VALUE as f32 + 0.5) as u8
}

/// Whether `position` has moved far enough from what was last sent for `value` to be worth
/// sending, which also covers the very first reading of a jack.
fn worth_sending(sent: Option<SentControlChange>, position: f32, value: u8) -> bool {
    match sent {
        Some(sent) => value != sent.value && (position - sent.position).abs() > MIDI_HYSTERESIS,
        None => true,
    }
}

/// The total resistance the next solve of a jack should settle for: nothing to settle when
/// the jack is empty, and as long as allowed when the total could not be determined.
fn expected_total(resistances: &ArmResistances) -> f32 {
    let disconnected = resistances
        .relative
        .iter()
        .all(|relative| relative.is_infinite());
    if disconnected { 0.0 } else { resistances.total }
}

fn jack_status(outcome: &SolveOutcome, position: Option<f32>) -> JackStatus {
    JackStatus {
        value: position.unwrap_or(0.0),
        resistances: outcome.resistances,
        voltages: outcome.voltages,
        pulls: outcome.pulls.map(ArmPull::from),
    }
}

/// Color the jack's LED shows: off while nothing usable is plugged in, otherwise the
/// jack's own color brightened with the pedal's position.
fn jack_color(jack: usize, position: Option<f32>) -> RGB8 {
    let Some(position) = position else {
        return RGB8::default();
    };

    let intensity = CONNECTED_LED_FLOOR + (1.0 - CONNECTED_LED_FLOOR) * position.clamp(0.0, 1.0);
    let color = JACK_COLORS[jack];

    RGB8 {
        r: (color.r as f32 * intensity) as u8,
        g: (color.g as f32 * intensity) as u8,
        b: (color.b as f32 * intensity) as u8,
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());

    info!("Starting WiFi access point");
    let access_point = AccessPointPeripherals {
        pio: peripherals.PIO1,
        dma: peripherals.DMA_CH0,
        power: peripherals.PIN_23,
        data: peripherals.PIN_24,
        chip_select: peripherals.PIN_25,
        clock: peripherals.PIN_29,
    };
    let stack = start_access_point(spawner, access_point, Irqs, AccessPointConfig::default()).await;
    spawn_web_server(spawner, stack);

    info!("Initializing pull switches");
    let mut switches = board::pull_switches(
        peripherals.SPI1,
        peripherals.PIN_14,
        peripherals.PIN_15,
        peripherals.PIN_11,
        peripherals.PIN_13,
    );
    unwrap!(switches.clear());

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
        .with_update_rate(ADC_UPDATE_RATE);
    unwrap!(adcs.init(adc_config).await);
    info!("ADC full scale: {}V", adcs.full_scale_voltage());

    info!("Initializing LED strip");
    let mut leds = board::leds(
        peripherals.PIO0,
        Irqs,
        peripherals.DMA_CH1,
        peripherals.PIN_6,
    );

    info!("Initializing USB MIDI");
    let (mut midi, mut usb_device) = UsbMidi::new(peripherals.USB, Irqs, UsbMidiConfig::default());

    let solver_config = ResistanceSolverConfig {
        solver: SolverConfig::from_voltage_noise(ADC_VOLTAGE_NOISE),
        tap_capacitance: board::INPUT_FILTER_CAPACITANCE,
        tap_series_resistance: board::INPUT_FILTER_RESISTANCE,
        ..Default::default()
    };
    let mut solver = ResistanceSolver::new(solver_config, switches, adcs);
    let jack_arms: [[ArmConfig; ARM_COUNT]; JACK_COUNT] = JACKS.map(|jack| jack.arms());
    let mut settings_receiver = unwrap!(INTERFACE.settings.receiver());
    let mut settings = settings_receiver.get().await;

    let detect = async {
        let mut ticker = Ticker::every(MINIMUM_SWEEP_INTERVAL);
        let mut rails = [None::<ArmRails>; JACK_COUNT];
        // What each jack measured last sweep, which is what the next one settles for.
        let mut expected_totals = [f32::NAN; JACK_COUNT];
        let mut sent = [None::<SentControlChange>; JACK_COUNT];
        let mut sweep = 0u32;

        loop {
            if let Some(changed) = settings_receiver.try_changed() {
                info!("Settings changed: {}", changed);
                settings = changed;
                // Everything a control change is built from may have changed with them, so
                // let every jack send again rather than work out which parts still match.
                sent = [None; JACK_COUNT];
            }
            leds.set_brightness(settings.led_brightness);

            let started = Instant::now();
            let mut jacks = [JackStatus::DISCONNECTED; JACK_COUNT];
            let mut measured_rails = false;

            for (index, arms) in jack_arms.iter().enumerate() {
                if rails[index].is_none() || sweep.is_multiple_of(RAIL_REFRESH_SWEEPS) {
                    measured_rails = true;
                    match solver.measure_rails(arms).await {
                        Ok(measured) => {
                            info!("Jack {} rails: {}", index, measured);
                            rails[index] = Some(measured);
                        }
                        Err(error) => {
                            warn!("Jack {} rail measurement failed: {}", index, error);
                            rails[index] = None;
                        }
                    }
                }

                let Some(jack_rails) = rails[index] else {
                    continue;
                };

                // A jack being unplugged mid-measurement leaves the readings of one solve
                // inconsistent with each other, which is a transient rather than a fault:
                // report the jack as disconnected and pick it up again next sweep.
                let outcome = match solver
                    .solve(arms, &jack_rails, expected_totals[index])
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        debug!("Jack {} did not solve: {}", index, error);
                        sent[index] = None;
                        expected_totals[index] = f32::NAN;
                        continue;
                    }
                };
                expected_totals[index] = expected_total(&outcome.resistances);

                let position = outcome
                    .resistances
                    .wiper_position(ArmResistances::DEFAULT_MAX_WIPER_RELATIVE);
                jacks[index] = jack_status(&outcome, position);

                if sweep.is_multiple_of(SWEEP_REPORT_SWEEPS) {
                    info!(
                        "Jack {}: {}, position {}",
                        index, outcome.resistances, position
                    );
                }

                if let Some(position) = position {
                    let jack_settings = &settings.jacks[index];
                    let value = control_value(position, jack_settings);

                    if worth_sending(sent[index], position, value) {
                        sent[index] = Some(SentControlChange { position, value });
                        let update = MidiUpdate {
                            channel: jack_settings.midi_channel,
                            controller: jack_settings.midi_controller,
                            value,
                        };

                        if MIDI_UPDATES.try_send(update).is_err() {
                            debug!("Dropped {}, the MIDI queue is full", update);
                        }
                    }
                } else {
                    sent[index] = None;
                }

                leds.set_color(index, jack_color(index, position));
            }

            INTERFACE.status.sender().send(Status {
                uptime_seconds: Instant::now().as_secs(),
                jacks,
            });
            leds.update().await;

            let elapsed = started.elapsed().as_millis();
            if measured_rails {
                info!("Sweep took {}ms, including a rail measurement", elapsed);
            } else if sweep.is_multiple_of(SWEEP_REPORT_SWEEPS) {
                info!("Sweep took {}ms", elapsed);
            }

            sweep = sweep.wrapping_add(1);
            ticker.next().await;
        }
    };

    let send_control_changes = async {
        loop {
            midi.wait_connection().await;
            info!("USB MIDI host connected");

            // Whatever queued up while no host was listening describes a position the
            // pedal has long moved on from.
            while MIDI_UPDATES.try_receive().is_ok() {}

            loop {
                let update = MIDI_UPDATES.receive().await;
                let Ok(channel) = MidiChannel::try_from(update.channel) else {
                    warn!("Ignoring {}, its MIDI channel is out of range", update);
                    continue;
                };

                let message = Message::ControlChange(
                    channel,
                    ControlFunction(U7::from_clamped(update.controller)),
                    U7::from_clamped(update.value),
                );
                if let Err(error) = midi.send_message(CableNumber::Cable0, message).await {
                    warn!("USB MIDI send failed: {}", error);
                    break;
                }
            }
        }
    };

    join3(usb_device.run(), send_control_changes, detect).await;
}
