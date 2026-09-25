#![no_std]
#![no_main]

use defmt::{debug, info, unwrap, warn};
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0, PIO1, USB};
use embassy_rp::{bind_interrupts, dma, pio, usb};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant};
use expad::board::{self, JACKS};
use expad::hal::adc::AdcChainConfig;
use expad::hal::led::RGB8;
use expad::hal::usb::{
    CableNumber, Channel as MidiChannel, ControlFunction, FromClamped, Message, U7, UsbMidi,
    UsbMidiConfig,
};
use expad::hal::wifi::{AccessPointConfig, AccessPointPeripherals, start_access_point};
use expad::topology::scanner::{JackScanner, SettleConfig};
use expad::topology::{JackMode, JackReport, MonitorConfig, SolverConfig};
use expad::web::{
    ArmPull, INTERFACE, JACK_COUNT, JackSettings, JackStatus, Settings, Status, spawn_web_server,
};

use {defmt_rtt as _, panic_probe as _};

/// ADC conversions per second (filter word 5). A reading takes three conversion periods to
/// settle after a channel change, 4.4 ms here against 10.2 ms at 315 Hz, and still resolves
/// 0.2 mV. Faster words turn much coarser - see docs/fast-tracking.md for the measurements.
const ADC_UPDATE_RATE: u32 = 819;

/// Standard deviation of a single reading at [`ADC_UPDATE_RATE`], measured on the PCB with
/// `adc_characterization` through a pedal (0.39 mV, 0.45 mV sample to sample). Every tolerance
/// the solver applies is derived from it.
const ADC_VOLTAGE_NOISE: f32 = 0.0005;

/// Color a jack's LED turns while its pedal moves or its switch is pressed, matching the
/// green of `INDICATOR_COLORS` in web/src/theme.ts.
const ACTIVE_LED_COLOR: RGB8 = RGB8 {
    r: 0x00,
    g: 0xC9,
    b: 0x50,
};

/// How long a jack's LED stays green after its pedal last moved far enough to send a new
/// control change. Bridges the gaps between the steps of a slowly moving pedal.
const MOVEMENT_INDICATION: Duration = Duration::from_millis(250);

/// Expression value from which a switch counts as pressed.
const SWITCH_PRESSED_VALUE: f32 = 0.5;

/// How often the web interface is sent a new status. Positions update far more often than
/// a browser can show, so this only bounds the WebSocket traffic.
const STATUS_INTERVAL: Duration = Duration::from_millis(33);

/// How often the LED strip is rewritten with the latest positions.
const LED_INTERVAL: Duration = Duration::from_millis(20);

/// How often the number of position updates each jack received is logged.
const RATE_REPORT_INTERVAL: Duration = Duration::from_secs(5);

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

/// What the last control change sent for a jack was, and the expression value it was
/// quantized from.
#[derive(Debug, Clone, Copy)]
struct SentControlChange {
    value: f32,
    control: u8,
}

/// Quantizes an expression value in `0.0..=1.0` to a control change value.
fn control_value(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * MIDI_MAX_VALUE as f32 + 0.5) as u8
}

/// Whether `value` has moved far enough from what was last sent for its control change to be
/// worth sending, which also covers the very first reading of a jack.
fn worth_sending(sent: Option<SentControlChange>, value: f32, control: u8) -> bool {
    match sent {
        Some(sent) => control != sent.control && (value - sent.value).abs() > MIDI_HYSTERESIS,
        None => true,
    }
}

fn jack_status(report: &JackReport, settings: &JackSettings) -> JackStatus {
    let [tip, ring, sleeve] = report.voltages;
    let mut status = JackStatus {
        mode: report.mode,
        position: report.position,
        value: report
            .position
            .map_or(0.0, |position| settings.value(position)),
        resistances: report.resistances,
        voltages: [tip, ring, sleeve, report.tip_switch_voltage],
        pulls: report.drives.map(ArmPull::from),
    };

    // An empty jack keeps its plug checks' voltages and drives, but nothing behind them.
    if report.mode == JackMode::Empty {
        status.resistances = JackStatus::DISCONNECTED.resistances;
        status.value = 0.0;
    }

    status
}

/// Hands the settings' wiper choices to the scanner.
fn apply_wiper_settings<const S: usize, const A: usize, const J: usize>(
    scanner: &mut JackScanner<'_, S, A, J>,
    settings: &Settings,
) {
    for (jack, jack_settings) in settings.jacks.iter().enumerate().take(J) {
        scanner.set_preferred_wiper(jack, jack_settings.wiper.arm());
    }
}

/// Color the jack's LED shows: off while nothing usable is plugged in, green while the pedal
/// moves (it `last_moved` within [`MOVEMENT_INDICATION`] of `now`) or its switch is pressed,
/// otherwise the jack's own color.
fn jack_color(
    report: &JackReport,
    settings: &JackSettings,
    last_moved: Option<Instant>,
    now: Instant,
) -> RGB8 {
    let Some(position) = report.position else {
        return RGB8::default();
    };

    let active = match report.mode {
        JackMode::Switch => settings.value(position) >= SWITCH_PRESSED_VALUE,
        _ => last_moved.is_some_and(|moved| now - moved < MOVEMENT_INDICATION),
    };

    if active {
        ACTIVE_LED_COLOR
    } else {
        settings.color.into()
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

    let monitor_config = MonitorConfig::new(
        SolverConfig::from_voltage_noise(ADC_VOLTAGE_NOISE),
        board::PULL_RESISTANCE,
    );
    let settle_config = SettleConfig {
        tap_capacitance: board::INPUT_FILTER_CAPACITANCE,
        tap_series_resistance: board::INPUT_FILTER_RESISTANCE,
        ..Default::default()
    };
    let mut scanner = JackScanner::new(
        switches,
        adcs,
        settle_config,
        JACKS.map(|jack| jack.config()),
        monitor_config,
    );
    let mut settings_receiver = unwrap!(INTERFACE.settings.receiver());
    let mut settings = settings_receiver.get().await;

    let detect = async {
        apply_wiper_settings(&mut scanner, &settings);
        leds.set_brightness(settings.led_brightness);

        let mut sent = [None::<SentControlChange>; JACK_COUNT];
        let mut last_moved = [None::<Instant>; JACK_COUNT];
        let mut modes = [JackMode::Empty; JACK_COUNT];
        let mut status_due = Instant::now();
        let mut leds_due = Instant::now();
        let mut rate_window_started = Instant::now();
        let mut position_updates = [0u32; JACK_COUNT];

        loop {
            if let Some(changed) = settings_receiver.try_changed() {
                info!("Settings changed: {}", changed);
                settings = changed;
                apply_wiper_settings(&mut scanner, &settings);
                leds.set_brightness(settings.led_brightness);
                // Everything a control change is built from may have changed with them, so
                // let every jack send again rather than work out which parts still match.
                sent = [None; JACK_COUNT];
            }

            let changed = match scanner.step().await {
                Ok(changed) => changed,
                Err(error) => {
                    warn!("Scanning the jacks failed: {}", error);
                    continue;
                }
            };

            for jack in (0..JACK_COUNT).filter(|&jack| changed[jack]) {
                let report = *scanner.report(jack);
                if report.mode != modes[jack] {
                    info!(
                        "Jack {}: {}, {}, tip switch {}V",
                        jack, report.mode, report.resistances, report.tip_switch_voltage
                    );
                    modes[jack] = report.mode;
                }
                let followed = matches!(
                    report.mode,
                    JackMode::Tracking | JackMode::Switch | JackMode::Rheostat
                );
                if followed {
                    position_updates[jack] += 1;
                }

                let jack_settings = &settings.jacks[jack];
                let value = report
                    .position
                    .map(|position| jack_settings.value(position));

                let Some(value) = value else {
                    sent[jack] = None;
                    continue;
                };

                let control = control_value(value);
                if worth_sending(sent[jack], value, control) {
                    // The first control change after plugging in or a settings change only
                    // tells the host where the pedal is, it did not move.
                    if sent[jack].is_some() {
                        last_moved[jack] = Some(Instant::now());
                    }
                    sent[jack] = Some(SentControlChange { value, control });
                    let update = MidiUpdate {
                        channel: jack_settings.midi_channel,
                        controller: jack_settings.midi_controller,
                        value: control,
                    };

                    if MIDI_UPDATES.try_send(update).is_err() {
                        debug!("Dropped {}, the MIDI queue is full", update);
                    }
                }
            }

            let now = Instant::now();
            if now >= status_due {
                status_due = now + STATUS_INTERVAL;
                INTERFACE.status.sender().send(Status {
                    jacks: core::array::from_fn(|jack| {
                        jack_status(scanner.report(jack), &settings.jacks[jack])
                    }),
                });
            }

            if now >= leds_due {
                leds_due = now + LED_INTERVAL;
                for (jack, jack_last_moved) in last_moved.iter().enumerate() {
                    let color = jack_color(
                        scanner.report(jack),
                        &settings.jacks[jack],
                        *jack_last_moved,
                        now,
                    );
                    leds.set_color(jack, color);
                }
                leds.update().await;
            }

            let window = now - rate_window_started;
            if window >= RATE_REPORT_INTERVAL {
                let window_millis = window.as_millis() as u32;
                let rates = position_updates.map(|updates| updates * 1000 / window_millis);
                info!("Position updates per second: {}", rates);
                position_updates = [0; JACK_COUNT];
                rate_window_started = now;
            }
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
