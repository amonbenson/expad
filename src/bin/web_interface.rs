#![no_std]
#![no_main]

use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{DMA_CH0, PIO1};
use embassy_rp::{dma, pio};
use embassy_time::{Duration, Instant, Ticker};
use expad::hal::wifi::{AccessPointConfig, AccessPointPeripherals, start_access_point};
use expad::topology::{ArmResistances, JackMode};
use expad::web::{ArmPull, INTERFACE, JACK_COUNT, JackStatus, Status, spawn_web_server};

use {defmt_rtt as _, panic_probe as _};

const STATUS_INTERVAL: Duration = Duration::from_millis(100);
const SWEEP_PERIOD_MILLISECONDS: u64 = 4000;

/// Pretended circuit the dummy voltages are derived from: a 10 kOhm potentiometer measured
/// between a pulled-up and a pulled-down arm. Resistances are in kOhm and currents in mA.
const DUMMY_TOTAL_RESISTANCE: f32 = 10.0;
const DUMMY_PULL_RESISTANCE: f32 = 1.0;
const HIGH_RAIL_VOLTAGE: f32 = 2.5;

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
        return JackStatus::DISCONNECTED;
    }

    let phase_offset = jack as u64 * SWEEP_PERIOD_MILLISECONDS / JACK_COUNT as u64;
    let phase = (uptime_milliseconds + phase_offset) % SWEEP_PERIOD_MILLISECONDS;
    let value = 1.0 - (2.0 * phase as f32 / SWEEP_PERIOD_MILLISECONDS as f32 - 1.0).abs();

    // Arm 0 is the wiper, so the two pot halves sit on arms 1 and 2, with the wiper `value`
    // of the way from arm 2's (the sleeve's) end, as `ArmResistances::wiper_position` reads it.
    let resistances = ArmResistances {
        relative: [0.0, 1.0 - value, value],
        total: DUMMY_TOTAL_RESISTANCE,
    };

    JackStatus {
        mode: JackMode::Tracking,
        position: Some(value),
        value,
        resistances,
        voltages: dummy_arm_voltages(resistances),
        pulls: [ArmPull::Floating, ArmPull::Up, ArmPull::Down],
    }
}

/// Voltages the arms of `resistances` would show while arm 1 is pulled up and arm 2 pulled down:
/// the driven arms drop the current across their pull resistors, and the floating arm 0 carries no
/// current, so its tap sits at the center node voltage.
fn dummy_arm_voltages(resistances: ArmResistances) -> [f32; 3] {
    let pulled_up_resistance = resistances.relative[1] * resistances.total;
    let pulled_down_resistance = resistances.relative[2] * resistances.total;
    let current = HIGH_RAIL_VOLTAGE
        / (2.0 * DUMMY_PULL_RESISTANCE + pulled_up_resistance + pulled_down_resistance);

    let pulled_up_voltage = HIGH_RAIL_VOLTAGE - current * DUMMY_PULL_RESISTANCE;
    let pulled_down_voltage = current * DUMMY_PULL_RESISTANCE;
    let center_voltage = pulled_down_voltage + current * pulled_down_resistance;

    [center_voltage, pulled_up_voltage, pulled_down_voltage]
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Starting WiFi access point");
    let peripherals = AccessPointPeripherals {
        pio: p.PIO1,
        dma: p.DMA_CH0,
        power: p.PIN_23,
        data: p.PIN_24,
        chip_select: p.PIN_25,
        clock: p.PIN_29,
    };
    let stack = start_access_point(spawner, peripherals, Irqs, AccessPointConfig::default()).await;
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
