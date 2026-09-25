//! Drives a [`JackMonitor`] from a simulated jack over simulated time: plugging and
//! unplugging, moving the pedal, end stops, other pedal types and noise.

#[allow(dead_code)]
mod star_network;

use expad_topology::{
    ARM_COUNT, Drive, JackMode, JackMonitor, MonitorConfig, Reading, SolverConfig, TIP, TIP_SWITCH,
};
use star_network::{Role, StarNetwork};

const HIGH_RAIL: f32 = 2.5;

/// What an undriven tap with nothing connected reads: whatever its filter capacitor holds.
const FLOATING_TAP: f32 = 1.0;

/// Time one reading takes on the board at 819 Hz, including the odd settling wait, in ms.
const READING_TIME: u64 = 5;

const RING: usize = 1;
const SLEEVE: usize = 2;

/// Standard deviation of the noise on every reading in the noisy tests, as measured.
const NOISE: f32 = SolverConfig::DEFAULT_VOLTAGE_NOISE;

/// Resistance of a potentiometer's wiper contact, in kΩ.
const WIPER_CONTACT: f32 = 0.05;

/// A potentiometer of `total` kΩ with its wiper on `wiper`, `position` of the way from the
/// track's start (the sleeve, if it is a track end) to its other end.
fn potentiometer(wiper: usize, position: f32, total: f32) -> [f32; ARM_COUNT] {
    let mut arms = [0.0; ARM_COUNT];
    let (start, end) = expad_topology::track_ends(wiper);
    arms[wiper] = WIPER_CONTACT;
    arms[start] = position * total;
    arms[end] = (1.0 - position) * total;
    arms
}

/// A jack with a 1 kΩ pull-up to 2.5 V and pull-down to ground on every contact, and a tip
/// switch touching the tip while nothing is plugged in.
struct SimulatedJack {
    plugged: bool,
    /// What is behind the plug; `None` for a cable with nothing at its other end.
    network: Option<[f32; ARM_COUNT]>,
    noise: f32,
    random_state: u32,
}

impl SimulatedJack {
    fn empty() -> Self {
        Self {
            plugged: false,
            network: None,
            noise: 0.0,
            random_state: 12345,
        }
    }

    fn plug(&mut self, network: Option<[f32; ARM_COUNT]>) {
        self.plugged = true;
        self.network = network;
    }

    fn voltage(&mut self, reading: &Reading) -> f32 {
        let clean = self.clean_voltage(reading);
        clean + self.noise * self.gaussian()
    }

    fn clean_voltage(&self, reading: &Reading) -> f32 {
        let rail = |drive: Drive| match drive {
            Drive::High => Some(HIGH_RAIL),
            Drive::Low => Some(0.0),
            Drive::Floating => None,
        };

        if reading.contact == TIP_SWITCH {
            let tip_switch = rail(reading.drives[TIP_SWITCH]);
            if self.plugged {
                return tip_switch.unwrap_or(FLOATING_TAP);
            }
            // Touching the tip: two driven contacts divide the rails through their pulls.
            return match (rail(reading.drives[TIP]), tip_switch) {
                (Some(tip), Some(tip_switch)) => (tip + tip_switch) / 2.0,
                (Some(driven), None) | (None, Some(driven)) => driven,
                (None, None) => FLOATING_TAP,
            };
        }

        let roles = core::array::from_fn(|arm| match reading.drives[arm] {
            Drive::High => Role::High,
            Drive::Low => Role::Low,
            Drive::Floating => Role::Floating,
        });

        match (self.plugged, self.network) {
            (true, Some(arms)) => {
                let network = StarNetwork::new(arms).with_rails(0.0, HIGH_RAIL);
                let voltage = network.tap_voltages(roles)[reading.contact];
                if voltage.is_nan() {
                    FLOATING_TAP
                } else {
                    voltage
                }
            }
            // Nothing behind the tap: a driven one sits exactly at its rail.
            _ => rail(reading.drives[reading.contact]).unwrap_or(FLOATING_TAP),
        }
    }

    /// Roughly normal noise with unit deviation, from a sum of uniform samples.
    fn gaussian(&mut self) -> f32 {
        let mut sum = 0.0;
        for _ in 0..12 {
            self.random_state = self
                .random_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            sum += (self.random_state >> 8) as f32 / (1u32 << 24) as f32;
        }
        sum - 6.0
    }
}

/// A monitor and its jack on a shared clock, taking a reading whenever the monitor wants one.
struct Harness {
    monitor: JackMonitor,
    jack: SimulatedJack,
    now: u64,
    readings: u32,
    position_updates: u32,
    mode_changes: u32,
}

impl Harness {
    fn new() -> Self {
        let mut harness = Self {
            monitor: JackMonitor::new(MonitorConfig::default()),
            jack: SimulatedJack::empty(),
            now: 0,
            readings: 0,
            position_updates: 0,
            mode_changes: 0,
        };
        // Rails and the first plug checks.
        harness.run_for(500);
        harness.reset_counters();
        harness
    }

    fn reset_counters(&mut self) {
        self.readings = 0;
        self.position_updates = 0;
        self.mode_changes = 0;
    }

    fn run_for(&mut self, duration: u64) {
        let end = self.now + duration;
        while self.now < end {
            let due_at = self.monitor.due_at();
            if due_at > self.now {
                self.now = due_at.min(end);
                continue;
            }

            let reading = self.monitor.reading();
            let voltage = self.jack.voltage(&reading);
            self.now += READING_TIME;
            let mode = self.monitor.report().mode;

            let changed = self.monitor.record(voltage, self.now);
            self.readings += 1;

            let report = self.monitor.report();
            if report.mode != mode {
                self.mode_changes += 1;
            } else if changed && report.mode == JackMode::Tracking {
                self.position_updates += 1;
            }
        }
    }

    /// Runs until the monitor reaches `mode`, returning how long that took, in ms.
    fn run_until(&mut self, mode: JackMode, limit: u64) -> u64 {
        let started = self.now;
        while self.monitor.report().mode != mode {
            assert!(
                self.now - started < limit,
                "not {mode:?} after {limit} ms: {:?}",
                self.monitor.report()
            );
            self.run_for(READING_TIME);
        }
        self.now - started
    }

    fn mode(&self) -> JackMode {
        self.monitor.report().mode
    }

    fn position(&self) -> f32 {
        self.monitor.report().position.expect("a position")
    }
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn polls_an_empty_jack_for_a_plug_without_keeping_its_adc_busy() {
    let mut harness = Harness::new();

    harness.run_for(1000);

    assert_eq!(harness.mode(), JackMode::Empty);
    assert!(harness.readings <= 21, "{} readings", harness.readings);
}

#[test]
fn tracks_a_newly_plugged_potentiometer_within_a_tenth_of_a_second() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.3, 10.0)));

    let delay = harness.run_until(JackMode::Tracking, 150);

    assert!(delay <= 120, "tracking after {delay} ms");
    harness.run_for(20);
    assert_close(harness.position(), 0.3, 1e-3);
}

#[test]
fn follows_the_wiper_with_one_reading_for_most_updates() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.5, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(50);
    harness.reset_counters();

    for step in 0..=100 {
        let position = step as f32 / 100.0;
        harness.jack.network = Some(potentiometer(TIP, position, 10.0));
        harness.run_for(20);

        assert_eq!(harness.mode(), JackMode::Tracking, "at {position}");
        assert_close(harness.position(), position, 1e-3);
    }

    assert_eq!(harness.mode_changes, 0);
    let wiper_share = harness.position_updates as f32 / harness.readings as f32;
    assert!(
        wiper_share >= 0.75,
        "only {wiper_share} of readings were the wiper"
    );
}

#[test]
fn keeps_tracking_through_both_end_stops() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(RING, 0.5, 10.0)));
    harness.run_until(JackMode::Tracking, 150);

    for position in [0.0, 1.0, 0.0, 0.5] {
        harness.jack.network = Some(potentiometer(RING, position, 10.0));
        harness.run_for(50);

        assert_eq!(harness.mode(), JackMode::Tracking);
        assert_close(harness.position(), position, 1e-3);
    }
}

/// The PCB's pedal fully closed: its mechanical stop leaves 9% of the track on the sleeve,
/// which is no end stop - the tip is clearly the wiper.
#[test]
fn tracks_a_pedal_whose_travel_stops_short_of_its_track_end() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.091, 10.85)));

    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(20);
    assert_close(harness.position(), 0.091, 1e-3);
}

/// Ring and sleeve shorted, the tip carrying the whole track: either the ring is the wiper
/// resting on the sleeve end, or the sleeve is the wiper resting on the ring end - at
/// opposite positions. The ring, the common wiring of the two, is taken for the wiper and
/// tracked straight away.
#[test]
fn tracks_an_ambiguous_end_stop_with_the_ring_as_wiper() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(RING, 0.0, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(20);
    assert_close(harness.position(), 0.0, 1e-3);

    harness.reset_counters();
    harness.jack.network = Some(potentiometer(RING, 0.4, 10.0));
    harness.run_for(50);
    assert_eq!(harness.mode_changes, 0);
    assert_close(harness.position(), 0.4, 1e-3);
}

/// On the PCB, solves of the pedal resting on its end stop came out up to 10% off in total
/// resistance, and tracking started from them lost the pedal at the first check of its track
/// ends. Standing in for that here: the pedal is identified as 9.8 kΩ, but measures 10.85 kΩ
/// once tracked.
#[test]
fn keeps_tracking_when_the_identifying_solve_was_off_in_total_resistance() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.0, 9.8)));
    harness.run_until(JackMode::Tracking, 150);
    harness.jack.network = Some(potentiometer(TIP, 0.0, 10.85));
    harness.reset_counters();

    for step in 0..=10 {
        harness.jack.network = Some(potentiometer(TIP, step as f32 * 0.1, 10.85));
        harness.run_for(30);
    }

    assert_eq!(harness.mode_changes, 0);
    assert_close(harness.position(), 1.0, 1e-3);
}

/// The ring is the wiper here, resting on the tip end; tip and ring read the same position
/// either way, so the tip is tracked as the wiper - until the pedal moves and the track the
/// ring was driven as turns out to change length. Solves taken while the pedal keeps moving
/// disagree between their pairs, so the right wiper is found once it slows down.
#[test]
fn corrects_a_wrong_wiper_guess_once_the_pedal_leaves_the_end_stop() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(RING, 1.0, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(20);
    assert_close(harness.position(), 1.0, 1e-2);

    let mut noticed_after = None;
    for step in 1..=20 {
        harness.jack.network = Some(potentiometer(RING, 1.0 - step as f32 * 0.025, 10.0));
        harness.run_for(20);
        if noticed_after.is_none() && harness.mode() == JackMode::Identifying {
            noticed_after = Some(step as f32 * 0.025);
        }
    }
    let noticed_after = noticed_after.expect("the wrong wiper to be noticed");
    assert!(
        noticed_after <= 0.05,
        "noticed after {noticed_after} of travel"
    );

    harness.run_until(JackMode::Tracking, 200);
    harness.run_for(20);
    assert_close(harness.position(), 0.5, 1e-3);
}

#[test]
fn reads_an_ambiguous_end_stop_with_the_preferred_wiper() {
    let mut harness = Harness::new();
    harness.monitor.set_preferred_wiper(Some(SLEEVE));
    // The sleeve is the wiper here, resting on the ring end.
    harness.jack.plug(Some(potentiometer(SLEEVE, 1.0, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(20);

    assert_close(harness.position(), 1.0, 1e-3);
}

#[test]
fn remembers_the_wiper_for_the_next_ambiguous_end_stop() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(SLEEVE, 0.4, 10.0)));
    harness.run_until(JackMode::Tracking, 150);

    harness.jack.plugged = false;
    harness.run_until(JackMode::Empty, 200);
    harness.jack.plug(Some(potentiometer(SLEEVE, 1.0, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(20);

    assert_close(harness.position(), 1.0, 1e-3);
}

#[test]
fn notices_an_unplugged_pedal_within_a_tenth_of_a_second() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.7, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(100);

    harness.jack.plugged = false;
    let delay = harness.run_until(JackMode::Empty, 200);

    assert!(delay <= 100, "empty after {delay} ms");
    assert_eq!(harness.monitor.report().position, None);
}

#[test]
fn identifies_a_rewired_pedal_again() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(TIP, 0.5, 10.0)));
    harness.run_until(JackMode::Tracking, 150);
    harness.run_for(100);

    // A polarity switch on the pedal moves the wiper to the ring.
    harness.jack.network = Some(potentiometer(RING, 0.25, 10.0));
    harness.run_for(200);

    assert_eq!(harness.mode(), JackMode::Tracking);
    assert_close(harness.position(), 0.25, 1e-3);
}

/// A mono sustain pedal: the plug's sleeve shorts the ring to it, and the pedal's switch
/// connects the tip to the sleeve while pressed.
#[test]
fn reports_a_switch_pedal_as_another_network_in_both_states() {
    let mut harness = Harness::new();
    let released = [f32::INFINITY, 0.0, 0.0];
    let pressed = [0.0, 0.0, 0.0];

    harness.jack.plug(Some(released));
    harness.run_until(JackMode::Other, 200);
    assert!(harness.monitor.report().resistances.relative[TIP].is_infinite());

    harness.jack.network = Some(pressed);
    harness.run_for(100);
    assert_eq!(harness.mode(), JackMode::Other);
    assert_eq!(harness.monitor.report().resistances.relative[TIP], 0.0);
    assert_eq!(harness.monitor.report().position, None);
}

#[test]
fn waits_on_an_open_cable_and_tracks_the_pedal_once_it_is_connected() {
    let mut harness = Harness::new();
    harness.jack.plug(None);
    harness.run_until(JackMode::Open, 200);
    harness.run_for(1000);
    assert_eq!(harness.mode(), JackMode::Open);

    harness.jack.network = Some(potentiometer(TIP, 0.2, 10.0));
    harness.run_until(JackMode::Tracking, 250);
    harness.run_for(20);
    assert_close(harness.position(), 0.2, 1e-3);
}

#[test]
fn stays_locked_on_a_still_pedal_through_measured_noise() {
    let mut harness = Harness::new();
    harness.jack.noise = NOISE;
    harness.jack.plug(Some(potentiometer(TIP, 0.35, 10.0)));
    harness.run_until(JackMode::Tracking, 300);
    harness.run_for(100);
    harness.reset_counters();

    let mut positions = Vec::new();
    for _ in 0..1000 {
        harness.run_for(10);
        positions.push(harness.position());
    }

    assert_eq!(harness.mode_changes, 0);
    let mean = positions.iter().sum::<f32>() / positions.len() as f32;
    let variance = positions
        .iter()
        .map(|p| (p - mean) * (p - mean))
        .sum::<f32>()
        / positions.len() as f32;
    assert_close(mean, 0.35, 1e-3);
    assert!(
        variance.sqrt() < 5e-4,
        "position deviation {}",
        variance.sqrt()
    );
}

/// A 250 kΩ volume pedal drops only 10 mV across each pull resistor, a few standard
/// deviations of noise - still enough to tell it apart from an unplugged jack.
#[test]
fn tracks_a_high_value_pedal_through_noise_and_notices_it_unplugged() {
    let mut harness = Harness::new();
    harness.jack.noise = NOISE;
    harness.jack.plug(Some(potentiometer(TIP, 0.5, 250.0)));
    harness.run_until(JackMode::Tracking, 400);
    harness.reset_counters();

    harness.run_for(5000);
    assert_eq!(harness.mode_changes, 0);
    assert_close(harness.position(), 0.5, 2e-3);

    harness.jack.plugged = false;
    harness.run_until(JackMode::Empty, 300);
}
