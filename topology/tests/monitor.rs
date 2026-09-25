//! Drives a [`JackMonitor`] from a simulated jack over simulated time: plugging and
//! unplugging, moving the pedal, end stops, other pedal types and noise.

#[allow(dead_code)]
mod star_network;

use expad_topology::{
    ARM_COUNT, Drive, JackMode, JackMonitor, MonitorConfig, RING, Reading, SLEEVE, SolverConfig,
    TIP, TIP_SWITCH,
};
use star_network::{Role, StarNetwork};

const HIGH_RAIL: f32 = 2.5;

/// What an undriven tap with nothing connected reads: whatever its filter capacitor holds.
const FLOATING_TAP: f32 = 1.0;

/// Time one reading takes on the board at 819 Hz, including the odd settling wait, in ms.
const READING_TIME: u64 = 5;

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
    /// Actual resistance of every contact's pull path, in kΩ - the monitor assumes 1 kΩ.
    pull_resistances: [f32; ARM_COUNT],
    noise: f32,
    random_state: u32,
}

impl SimulatedJack {
    fn empty() -> Self {
        Self {
            plugged: false,
            network: None,
            pull_resistances: [1.0; ARM_COUNT],
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
                let network = StarNetwork::new(arms)
                    .with_rails(0.0, HIGH_RAIL)
                    .with_pull_resistances(self.pull_resistances);
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

/// Resistance between tip and sleeve of a two-wire pedal behind a mono (TS) plug, whose sleeve
/// shorts the jack's ring to its sleeve.
fn mono(tip_sleeve: f32) -> [f32; ARM_COUNT] {
    [tip_sleeve, 0.0, 0.0]
}

/// The same behind a stereo (TRS) plug, with the ring left unconnected.
fn stereo(tip_sleeve: f32) -> [f32; ARM_COUNT] {
    [tip_sleeve, f32::INFINITY, 0.0]
}

/// A mono sustain pedal: released it leaves the tip open, pressed it shorts it to the sleeve.
/// A change shows within 20 ms, even when the plug and ring checks come due in between.
#[test]
fn follows_a_switch_behind_a_mono_plug_with_single_readings() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(mono(f32::INFINITY)));
    harness.run_until(JackMode::Switch, 200);
    harness.run_for(20);
    assert_close(harness.position(), 0.0, 0.0);
    harness.reset_counters();

    for _ in 0..10 {
        harness.jack.network = Some(mono(0.0));
        harness.run_for(20);
        assert_close(harness.position(), 1.0, 0.0);

        harness.jack.network = Some(mono(f32::INFINITY));
        harness.run_for(20);
        assert_close(harness.position(), 0.0, 0.0);
    }

    assert_eq!(harness.mode_changes, 0);
    assert_eq!(harness.mode(), JackMode::Switch);

    harness.jack.plugged = false;
    let delay = harness.run_until(JackMode::Empty, 200);
    assert!(delay <= 60, "empty after {delay} ms");
}

/// The plug checks between a switch's readings drive the tip low for a moment; the report
/// keeps showing how the switch is followed, so nothing flickers between the two.
#[test]
fn reports_steady_drives_while_following_a_switch() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(mono(0.0)));
    harness.run_until(JackMode::Switch, 200);

    let followed = [Drive::High, Drive::Floating, Drive::Low, Drive::Floating];
    let mut plug_checks = 0;
    for _ in 0..40 {
        if harness.monitor.reading().contact == TIP_SWITCH {
            plug_checks += 1;
        }
        harness.run_for(READING_TIME);
        assert_eq!(harness.monitor.report().drives, followed);
    }

    assert!(plug_checks >= 3, "only {plug_checks} plug checks");
}

/// A stereo sustain pedal released reads like a plug with nothing behind it; pressing it has
/// to be noticed straight away all the same.
#[test]
fn notices_a_stereo_switch_pressed_behind_a_plug_that_looked_open() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(stereo(f32::INFINITY)));
    harness.run_until(JackMode::Open, 200);
    assert_eq!(harness.monitor.report().position, None);

    harness.jack.network = Some(stereo(0.0));
    let delay = harness.run_until(JackMode::Switch, 100);
    assert!(delay <= 60, "switch after {delay} ms");
    harness.run_for(20);
    assert_close(harness.position(), 1.0, 0.0);
    harness.reset_counters();

    for _ in 0..10 {
        harness.jack.network = Some(stereo(f32::INFINITY));
        harness.run_for(20);
        assert_close(harness.position(), 0.0, 0.0);

        harness.jack.network = Some(stereo(0.0));
        harness.run_for(20);
        assert_close(harness.position(), 1.0, 0.0);
    }
    assert_eq!(harness.mode_changes, 0);
}

/// On the PCB, a pressed switch behind a mono plug was dropped: the ring check computes the
/// sleeve's voltage from the tip's pull drop, and the two pull paths differ by a few ohms
/// (0.1% resistors, switch on-resistance) - a few millivolts at the full current. Standing in
/// for that: a sleeve pull 4 Ω above the tip's.
#[test]
fn keeps_following_a_shorted_switch_despite_mismatched_pulls() {
    let mut harness = Harness::new();
    harness.jack.pull_resistances = [1.0, 1.0, 1.004];
    harness.jack.plug(Some(mono(0.0)));
    harness.run_until(JackMode::Switch, 200);
    harness.reset_counters();

    harness.run_for(2000);
    assert_eq!(harness.mode_changes, 0);
    assert_close(harness.position(), 1.0, 0.0);
}

/// Every reading of an open stereo plug looks for tip and sleeve connecting, so noise must not
/// pass for a pressed switch; and a switch must not flicker.
#[test]
fn keeps_switches_steady_through_measured_noise() {
    let mut harness = Harness::new();
    harness.jack.noise = NOISE;
    harness.jack.plug(Some(stereo(f32::INFINITY)));
    harness.run_until(JackMode::Open, 300);
    harness.reset_counters();
    harness.run_for(10_000);
    assert_eq!(harness.mode_changes, 0);

    harness.jack.plugged = false;
    harness.run_until(JackMode::Empty, 200);
    harness.jack.plug(Some(mono(f32::INFINITY)));
    harness.run_until(JackMode::Switch, 300);
    harness.reset_counters();
    for pressed in [false, true, false, true] {
        harness.jack.network = Some(mono(if pressed { 0.0 } else { f32::INFINITY }));
        harness.run_for(20);
        for _ in 0..200 {
            harness.run_for(10);
            assert_close(harness.position(), if pressed { 1.0 } else { 0.0 }, 0.0);
        }
    }
    assert_eq!(harness.mode_changes, 0);
}

/// A 25 kΩ rheostat behind a stereo plug, swept up and back down: read against its standard
/// full scale once it has shown it.
#[test]
fn follows_a_rheostat_behind_a_stereo_plug() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(stereo(5.0)));
    harness.run_until(JackMode::Rheostat, 200);

    for step in 0..=20 {
        harness.jack.network = Some(stereo(5.0 + step as f32));
        harness.run_for(10);
    }
    assert_close(harness.position(), 1.0, 1e-3);

    harness.jack.network = Some(stereo(12.5));
    harness.run_for(20);
    assert_eq!(harness.mode(), JackMode::Rheostat);
    assert_close(harness.position(), 0.5, 1e-3);

    // Reported as the tip's arm, with the star point on the sleeve and the ring isolated.
    let resistances = harness.monitor.report().resistances;
    let [tip, ring, sleeve] = resistances.relative;
    assert_close(tip * resistances.total, 12.5, 0.05);
    assert!(ring.is_infinite(), "ring at {ring}");
    assert_close(sleeve, 0.0, 0.0);
}

/// Behind a mono plug a rheostat reads like a ring-wiper potentiometer resting on its heel
/// end stop, until it moves: only its total resistance changes with it.
#[test]
fn tells_a_rheostat_behind_a_mono_plug_from_a_potentiometer_once_it_moves() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(mono(5.0)));
    harness.run_until(JackMode::Tracking, 200);

    for step in 0..=20 {
        harness.jack.network = Some(mono(5.0 + step as f32));
        harness.run_for(20);
    }
    assert_eq!(harness.mode(), JackMode::Rheostat);
    assert_close(harness.position(), 1.0, 1e-3);

    harness.jack.network = Some(mono(12.5));
    harness.run_for(20);
    assert_close(harness.position(), 0.5, 1e-3);
}

/// The potentiometer that same end stop could also be: it stays one all the way off the stop.
#[test]
fn keeps_a_ring_wiper_potentiometer_leaving_its_heel_end_stop_a_potentiometer() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(potentiometer(RING, 0.0, 10.0)));
    harness.run_until(JackMode::Tracking, 200);
    harness.reset_counters();

    for step in 0..=50 {
        let position = step as f32 / 50.0;
        harness.jack.network = Some(potentiometer(RING, position, 10.0));
        harness.run_for(20);
        assert_eq!(harness.mode(), JackMode::Tracking, "at {position}");
        assert_close(harness.position(), position, 1e-3);
    }
    assert_eq!(harness.mode_changes, 0);
}

/// A rheostat plugged in at its zero end reads as a closed switch, until it moves.
#[test]
fn turns_a_closed_switch_into_a_rheostat_once_it_moves() {
    let mut harness = Harness::new();
    harness.jack.plug(Some(mono(0.0)));
    harness.run_until(JackMode::Switch, 200);

    for step in 1..=20 {
        harness.jack.network = Some(mono(step as f32 * 0.5));
        harness.run_for(10);
    }

    assert_eq!(harness.mode(), JackMode::Rheostat);
    assert_close(harness.position(), 1.0, 1e-3);
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

/// A stereo cable left in the jack and plugged into a pedal: sliding in, the plug connects tip
/// and sleeve through part of the pedal before the ring, which reads like a rheostat. Once the
/// ring connects, the pedal is identified again; and once it is unplugged at the far end, the
/// next pedal is not taken for a rheostat because of it.
#[test]
fn identifies_a_pedal_plugged_into_a_cable_already_in_the_jack() {
    let mut harness = Harness::new();
    harness.jack.plug(None);
    harness.run_until(JackMode::Open, 200);

    for step in 0..20 {
        harness.jack.network = Some(stereo(2.0 + step as f32 * 0.4));
        harness.run_for(10);
    }
    assert_eq!(harness.mode(), JackMode::Rheostat);

    harness.jack.network = Some(potentiometer(TIP, 0.3, 10.0));
    let delay = harness.run_until(JackMode::Tracking, 500);
    assert!(delay <= 250, "tracking after {delay} ms");
    harness.run_for(20);
    assert_close(harness.position(), 0.3, 1e-3);

    harness.jack.network = None;
    harness.run_until(JackMode::Open, 300);
    harness.jack.network = Some(stereo(0.0));
    harness.run_until(JackMode::Switch, 200);
    harness.run_for(20);
    assert_close(harness.position(), 1.0, 0.0);
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
