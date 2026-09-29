#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{DMA_CH0, PIO1};
use embassy_rp::{dma, pio};
use embassy_time::{Duration, Instant, Ticker};
use expad::board::{JACK_COUNT, PULL_RESISTANCE, REFERENCE_VOLTAGE};
use expad::hal::wifi::{AccessPointConfig, AccessPointPeripherals, start_access_point};
use expad::topology::{
    ARM_COUNT, ArmResistances, CONTACT_COUNT, JackMode, RING, SLEEVE, TIP, TIP_SWITCH,
};
use expad::web::{ArmPull, INTERFACE, JackStatus, Status, spawn_web_server};

use {defmt_rtt as _, panic_probe as _};

const STATUS_INTERVAL: Duration = Duration::from_millis(100);
const SWEEP_PERIOD_MILLISECONDS: u64 = 4000;

/// Pretended potentiometer the dummy voltages are derived from, measured between a pulled-up
/// and a pulled-down arm through the board's pulls, in kΩ.
const DUMMY_TOTAL_RESISTANCE: f32 = 10.0;

bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Web Interface"),
    embassy_rp::binary_info::rp_program_description!(
        c"Serves the web interface over a WiFi access point with dummy status data and logs settings changes"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

/// Pretends a potentiometer is plugged into every jack but the last, sweeping each one
/// back and forth with a phase offset per jack.
fn dummy_jack_status(uptime_milliseconds: u64, jack: usize) -> JackStatus {
    if jack == JACK_COUNT - 1 {
        // Plug checks: the tip switch pulled up against the tip pulled down, halfway between.
        return JackStatus {
            voltages: [0.0, f32::NAN, f32::NAN, REFERENCE_VOLTAGE / 2.0],
            pulls: [
                ArmPull::Down,
                ArmPull::Floating,
                ArmPull::Floating,
                ArmPull::Up,
            ],
            ..JackStatus::DISCONNECTED
        };
    }

    let phase_offset = jack as u64 * SWEEP_PERIOD_MILLISECONDS / JACK_COUNT as u64;
    let phase = (uptime_milliseconds + phase_offset) % SWEEP_PERIOD_MILLISECONDS;
    let value = 1.0 - (2.0 * phase as f32 / SWEEP_PERIOD_MILLISECONDS as f32 - 1.0).abs();

    // The tip is the wiper, so the two track halves sit on ring and sleeve, with the wiper
    // `value` of the way from the sleeve's end, as `ArmResistances::position_with_wiper` reads it.
    let mut relative = [0.0; ARM_COUNT];
    relative[RING] = 1.0 - value;
    relative[SLEEVE] = value;
    let resistances = ArmResistances {
        relative,
        total: DUMMY_TOTAL_RESISTANCE,
    };

    JackStatus {
        mode: JackMode::Tracking,
        position: Some(value),
        value,
        resistances,
        voltages: dummy_contact_voltages(resistances),
        pulls: [
            ArmPull::Floating,
            ArmPull::Up,
            ArmPull::Down,
            ArmPull::Floating,
        ],
    }
}

/// Voltages the contacts of `resistances` would show while the ring is pulled up and the
/// sleeve pulled down: the driven arms drop the current across their pull resistors, and the
/// floating tip carries no current, so its tap sits at the star point voltage. The tip switch
/// keeps the high rail from the plug check that found the plug.
fn dummy_contact_voltages(resistances: ArmResistances) -> [f32; CONTACT_COUNT] {
    let pulled_up_resistance = resistances.absolute(RING);
    let pulled_down_resistance = resistances.absolute(SLEEVE);
    let current =
        REFERENCE_VOLTAGE / (2.0 * PULL_RESISTANCE + pulled_up_resistance + pulled_down_resistance);

    let mut voltages = [0.0; CONTACT_COUNT];
    voltages[RING] = REFERENCE_VOLTAGE - current * PULL_RESISTANCE;
    voltages[SLEEVE] = current * PULL_RESISTANCE;
    voltages[TIP] = voltages[SLEEVE] + current * pulled_down_resistance;
    voltages[TIP_SWITCH] = REFERENCE_VOLTAGE;
    voltages
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());

    info!("Starting WiFi access point");
    let radio = AccessPointPeripherals {
        pio: peripherals.PIO1,
        dma: peripherals.DMA_CH0,
        power: peripherals.PIN_23,
        data: peripherals.PIN_24,
        chip_select: peripherals.PIN_25,
        clock: peripherals.PIN_29,
    };
    let stack = start_access_point(spawner, radio, Irqs, AccessPointConfig::default()).await;
    spawn_web_server(spawner, stack);

    let mut settings = unwrap!(INTERFACE.settings.receiver());
    let publish_status = async {
        let mut ticker = Ticker::every(STATUS_INTERVAL);
        loop {
            let uptime = Instant::now();
            INTERFACE.status.sender().send(Status {
                jacks: core::array::from_fn(|jack| dummy_jack_status(uptime.as_millis(), jack)),
            });
            ticker.next().await;
        }
    };
    let log_settings = async {
        loop {
            info!("Settings changed: {}", settings.changed().await);
        }
    };
    join(publish_status, log_settings).await;
}
