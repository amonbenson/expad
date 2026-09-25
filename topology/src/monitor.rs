//! Watches one jack over time: notices a plug going in or out, identifies what is plugged in
//! with full solves, and then follows it with single readings.
//!
//! [`JackMonitor`] hands out one ADC reading at a time and takes its voltage back, so a
//! scheduler can interleave jacks on different ADCs, and tests can drive it from a model.
//!
//! Every jack runs through the same states, and reports the [`JackMode`] its state stands for:
//!
//! - rails: every arm's tap averaged while all three are driven high, then low - at start-up,
//!   and ahead of a solve once they are old;
//! - empty: the tip switch polled for a plug;
//! - solve: pairs of arms driven against each other until the network is known, then
//!   classified as a [`Network`], which decides what comes next;
//! - confirm plug: a solve found no current anywhere, so the tip switch tells an unplugged jack
//!   from a plug with nothing conducting behind it;
//! - follow: a potentiometer's wiper read between its driven track ends, or the tip read against
//!   the sleeve for a switch, a rheostat or an open plug. The checks interleaved with following
//!   send the jack back to be identified when it no longer behaves like what was identified,
//!   and to empty when the plug is gone.

use crate::config::SolverConfig;
use crate::measurement::{ArmDrive, PairMeasurement, PairVoltages};
use crate::resistances::{ARM_COUNT, ArmResistances, Network, RING, SLEEVE, TIP, track_ends};
use crate::sequence::{PAIR_SEQUENCE, SolveSequence, SolveStep};

/// Contacts of one jack that can be driven and read: its three arms plus the tip's
/// normalling contact.
pub const CONTACT_COUNT: usize = ARM_COUNT + 1;

/// Index of the tip switch contact among a jack's contacts.
pub const TIP_SWITCH: usize = ARM_COUNT;

/// Fraction of the way from the low to the high rail above which the tip switch shows a plug
/// during a plug check: without one it touches the pulled-down tip and the two pulls divide
/// the rails in half, with one it is isolated and reads its own rail.
const PLUG_THRESHOLD: f32 = 0.75;

/// Standard values of rheostat pedals, in kΩ, the full scale a rheostat's resistance is read
/// against.
const RHEOSTAT_VALUES: [f32; 10] = [1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];

/// How far a rheostat may measure above its standard value before the next larger one is
/// taken as its full scale: its own tolerance.
const RHEOSTAT_TOLERANCE: f32 = 1.25;

/// How one contact is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Drive {
    High,
    Low,
    Floating,
}

/// One ADC reading a monitor needs: how to drive the jack's contacts, and which one to read.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Reading {
    pub drives: [Drive; CONTACT_COUNT],
    pub contact: usize,

    /// Resistance the floating contacts charge their tap capacitance through, in kΩ, once
    /// `drives` differ from what the jack was driven with before: `0.0` if only driven taps
    /// matter, `f32::NAN` if unknown.
    pub settle_resistance: f32,
}

/// The tip pulled low and the tip switch high, reading the tip switch.
const PLUG_CHECK: Reading = {
    let mut drives = [Drive::Floating; CONTACT_COUNT];
    drives[TIP] = Drive::Low;
    drives[TIP_SWITCH] = Drive::High;
    Reading {
        drives,
        contact: TIP_SWITCH,
        settle_resistance: 0.0,
    }
};

/// What a monitor currently makes of its jack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum JackMode {
    /// No plug in the jack.
    Empty,
    /// Full solves until the network is known: just plugged in, or following lost it.
    Identifying,
    /// A potentiometer, followed by reading its wiper against its two track ends.
    Tracking,
    /// A switch between tip and sleeve (a sustain pedal), followed by reading the tip.
    Switch,
    /// A variable resistor between tip and sleeve (a two-wire expression pedal), followed by
    /// reading the tip.
    Rheostat,
    /// Some other network, solved in full again and again.
    Other,
    /// A plug with nothing conducting behind it, watched for tip and sleeve connecting.
    Open,
}

/// Everything a monitor currently knows about its jack.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct JackReport {
    pub mode: JackMode,
    /// Position in `0.0..=1.0`: a potentiometer's wiper as
    /// [`ArmResistances::position_with_wiper`] defines it, a switch's 0 open and 1 closed, or
    /// a rheostat's resistance against its full scale.
    pub position: Option<f32>,
    pub resistances: ArmResistances,
    /// Last voltage read at each arm's tap, in V.
    pub voltages: [f32; ARM_COUNT],
    /// How each contact (the arms, then the tip switch) is driven for the reading the monitor
    /// is on: that of its last reading, except for the plug checks interleaved with following,
    /// which briefly drive the tip low and the tip switch high.
    pub drives: [Drive; CONTACT_COUNT],
    /// Last plug check's reading of the tip switch, in V: half the rails without a plug, the
    /// high rail with one.
    pub tip_switch_voltage: f32,
}

impl JackReport {
    pub const EMPTY: Self = Self {
        mode: JackMode::Empty,
        position: None,
        resistances: ArmResistances::DISCONNECTED,
        voltages: [f32::NAN; ARM_COUNT],
        drives: [Drive::Floating; CONTACT_COUNT],
        tip_switch_voltage: f32::NAN,
    };
}

/// Timing and tolerances of a [`JackMonitor`]. Times are in ms.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MonitorConfig {
    pub solver: SolverConfig,

    /// Series resistance each arm is pulled up and pulled down through, in kΩ.
    pub pull_up_resistances: [f32; ARM_COUNT],
    pub pull_down_resistances: [f32; ARM_COUNT],

    /// Largest relative resistance an arm can have and still pass as a potentiometer's wiper,
    /// which sits at the star point and so contributes little more than its own contact and
    /// lead resistance.
    pub max_wiper_relative: f32,
    /// Largest relative resistance a track end can have and still count as shorted to the
    /// star point by a wiper resting on it, and a mono plug's ring as shorted to the sleeve.
    /// Far below the wiper's own allowance: a pedal whose travel ends 9% short of its track (a
    /// common mechanical stop) is a potentiometer with a clear wiper, not an end stop.
    pub end_stop_relative: f32,

    /// How often an empty jack is checked for a plug, and a jack followed through its tip
    /// for the plug still being there.
    pub plug_check_interval: u64,
    /// How often a plug with nothing conducting behind it is solved in full, for networks
    /// that do not connect tip and sleeve.
    pub open_solve_interval: u64,
    /// How often a mono plug's ring is checked for still being shorted to the sleeve.
    pub ring_check_interval: u64,
    /// Fraction of the sleeve's pull drop a mono plug's ring may read away from the sleeve
    /// voltage the tip implies, on top of the noise: that estimate takes both pull paths to
    /// be equal, which 0.1% resistors and a few ohms of switch resistance are not - a few
    /// millivolts at the full current of a closed switch, which dropped it on the PCB.
    pub ring_check_fraction: f32,
    /// How often a tracked potentiometer's track ends are read, alternating between them, to
    /// check it is still there and unchanged.
    pub end_tap_interval: u64,
    /// Age after which the rails are measured again before the next full solve.
    pub rail_refresh_interval: u64,
    /// Readings averaged into every rail voltage.
    pub rail_samples: u32,

    /// Total resistance a newly plugged network is assumed to have for settling, in kΩ,
    /// until a solve has measured it. Covers common pedals; a far larger one fails its first
    /// solves and then settles for as long as allowed.
    pub initial_total: f32,

    /// Consecutive rejected solves after which the last total resistance is no longer trusted
    /// for settling. A pedal moving between pairs gets rejected once or twice; one that keeps
    /// failing may have been swapped for a network that needs far longer to settle.
    pub trusted_failed_solves: u32,

    /// How far a tracked track end's pull drop may stray from the expected one, in standard
    /// deviations of a reading's noise or as a fraction of that drop, whichever is wider. The
    /// drop stays constant while the wiper moves, vanishes when the pedal is unplugged, and
    /// shifts with the wiper's travel when the wiper was guessed wrong at an end stop - so
    /// the fraction is about how far such a pedal moves before it is corrected.
    pub end_drop_sigmas: f32,
    pub end_drop_fraction: f32,

    /// The same fraction for a track end's first reading, which is checked against the drop
    /// the solve implied. Solves of a pedal on an end stop were seen 10% off in total
    /// resistance, so this only has to tell a pedal apart from an unplugged jack (no drop).
    pub first_end_drop_fraction: f32,

    /// Standard deviations of noise the wiper may read outside the span of its track ends,
    /// and a mono plug's ring away from the sleeve.
    pub wiper_range_sigmas: f32,

    /// Largest resistance between tip and sleeve that counts as a closed switch, in kΩ.
    pub closed_resistance: f32,
    /// Standard deviations of current noise the tip's current has to exceed for tip and
    /// sleeve to count as connected. Checked on every reading of an open plug, so it has to
    /// be far rarer to trip on noise than a solve's one-off check; 6 still resolves ~800 kΩ.
    pub element_open_sigmas: f32,
    /// Consecutive readings between closed and open after which a tip-sleeve element is a
    /// rheostat rather than a switch caught mid-bounce.
    pub rheostat_readings: u32,
    /// Change of the total resistance, as a fraction, that tells a rheostat behind a mono plug
    /// from a potentiometer resting on its end stop with ring and sleeve shorted: the same
    /// network until the pedal moves, but only the rheostat's total changes with it.
    pub end_stop_change_fraction: f32,
}

impl MonitorConfig {
    pub fn new(solver: SolverConfig, pull_resistance: f32) -> Self {
        Self {
            solver,
            pull_up_resistances: [pull_resistance; ARM_COUNT],
            pull_down_resistances: [pull_resistance; ARM_COUNT],
            max_wiper_relative: 0.1,
            end_stop_relative: 0.02,
            plug_check_interval: 50,
            open_solve_interval: 1000,
            ring_check_interval: 100,
            ring_check_fraction: 0.01,
            end_tap_interval: 25,
            rail_refresh_interval: 30_000,
            rail_samples: 8,
            initial_total: 100.0,
            trusted_failed_solves: 2,
            end_drop_sigmas: 6.0,
            end_drop_fraction: 0.03,
            first_end_drop_fraction: 0.25,
            wiper_range_sigmas: 6.0,
            closed_resistance: 0.2,
            element_open_sigmas: 6.0,
            rheostat_readings: 3,
            end_stop_change_fraction: 0.2,
        }
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self::new(
            SolverConfig::default(),
            SolverConfig::DEFAULT_PULL_RESISTANCE,
        )
    }
}

/// Rail voltages every arm's tap reads while all three are driven alike, so no current flows.
#[derive(Debug, Clone, Copy)]
struct Rails {
    high: [f32; ARM_COUNT],
    low: [f32; ARM_COUNT],
    measured_at: u64,
}

/// What a monitor is doing with its jack; see the module documentation.
#[derive(Debug, Clone, Copy)]
enum State {
    /// Averaging every arm's tap with all arms driven alike, then going on as `then` says.
    Rails {
        measurement: RailMeasurement,
        then: AfterRails,
    },
    /// No plug: the tip switch is checked every [`MonitorConfig::plug_check_interval`].
    Empty,
    /// A full solve, reported as `mode`: identifying, or solving an open plug or another
    /// network again.
    Solve { solve: Solve, mode: JackMode },
    /// A solve found no current anywhere, reported as `mode` until the tip switch tells an
    /// unplugged jack from an open plug.
    ConfirmPlug { mode: JackMode },
    /// Following what a solve identified with single readings.
    Follow(Follower),
}

impl State {
    fn mode(&self) -> JackMode {
        match self {
            State::Rails {
                then: AfterRails::Empty,
                ..
            }
            | State::Empty => JackMode::Empty,
            State::Rails {
                then: AfterRails::Solve(mode),
                ..
            }
            | State::Solve { mode, .. }
            | State::ConfirmPlug { mode } => *mode,
            State::Follow(follower) => follower.mode(),
        }
    }
}

/// Where a rails measurement goes on to.
#[derive(Debug, Clone, Copy)]
enum AfterRails {
    /// Start-up: poll for a plug.
    Empty,
    /// Measured again ahead of a solve, which is reported as this mode.
    Solve(JackMode),
}

/// Progress of a rails measurement: averaging `arm`'s tap with every arm at `level`, high
/// first.
#[derive(Debug, Clone, Copy)]
struct RailMeasurement {
    level: Drive,
    arm: usize,
    sample: u32,
    sum: f32,
    high: [f32; ARM_COUNT],
    low: [f32; ARM_COUNT],
}

impl RailMeasurement {
    const START: Self = Self {
        level: Drive::High,
        arm: 0,
        sample: 0,
        sum: 0.0,
        high: [f32::NAN; ARM_COUNT],
        low: [f32::NAN; ARM_COUNT],
    };
}

/// A full solve, reading the three taps of each pair in turn.
#[derive(Debug, Clone, Copy)]
struct Solve {
    sequence: SolveSequence,
    pair: (usize, usize),
    voltages: [f32; ARM_COUNT],
    read: usize,
}

impl Solve {
    fn new(config: SolverConfig) -> Self {
        let (high, low) = PAIR_SEQUENCE[0];
        Self::measuring(SolveSequence::new(config), high, low)
    }

    fn measuring(sequence: SolveSequence, high: usize, low: usize) -> Self {
        Self {
            sequence,
            pair: (high, low),
            voltages: [f32::NAN; ARM_COUNT],
            read: 0,
        }
    }
}

/// Two arms driven against each other, `high` high and `low` low, while single contacts are
/// read.
#[derive(Debug, Clone, Copy)]
struct Follower {
    high: usize,
    low: usize,
    /// Contact the next reading is of.
    next: usize,
    kind: Followed,
}

impl Follower {
    fn drives(&self) -> [Drive; CONTACT_COUNT] {
        let mut drives = pair_drives(self.high, self.low);
        if let Followed::TipSleeve(tip_sleeve) = &self.kind
            && !tip_sleeve.ring_check.shorted
        {
            drives[RING] = Drive::Low;
        }
        drives
    }

    fn mode(&self) -> JackMode {
        match &self.kind {
            Followed::Potentiometer(_) => JackMode::Tracking,
            Followed::TipSleeve(tip_sleeve) => match tip_sleeve.element {
                Element::Open { .. } => JackMode::Open,
                Element::Switch { .. } => JackMode::Switch,
                Element::Rheostat => JackMode::Rheostat,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Followed {
    /// A potentiometer with its track ends driven, reading its wiper between them.
    Potentiometer(Track),
    /// Whatever connects tip and sleeve, with the tip driven high and the sleeve low, reading
    /// the resistance between them off the tip's pull drop.
    TipSleeve(TipSleeve),
}

#[derive(Debug, Clone, Copy)]
struct Track {
    wiper: usize,
    low_end: TrackEnd,
    high_end: TrackEnd,
    /// When a track end is read again, alternating between them, to check the pedal is still
    /// there and unchanged.
    end_check: Periodic,
    next_end_is_high: bool,
}

/// One end of a tracked potentiometer's track.
#[derive(Debug, Clone, Copy)]
struct TrackEnd {
    /// Last voltage read at its tap, in V, `NaN` until its first reading.
    voltage: f32,
    /// Pull drop it is expected to show, in V: implied by the solve (`NaN` if it could not),
    /// then taken from the first reading and refined by every one that agrees.
    expected_drop: f32,
}

impl TrackEnd {
    fn read(&self) -> bool {
        !self.voltage.is_nan()
    }
}

#[derive(Debug, Clone, Copy)]
struct TipSleeve {
    element: Element,
    /// When the tip switch is read to check the plug is still in.
    plug_check: Periodic,
    ring_check: RingCheck,
    /// Voltage the sleeve's tap is at, from the last tip reading.
    sleeve_voltage: f32,
}

/// What connects tip and sleeve.
#[derive(Debug, Clone, Copy)]
enum Element {
    /// Nothing is known to: the first conduction sends the jack to be identified, and it is
    /// solved in full every so often, for networks that leave tip and sleeve apart.
    Open { solve: Periodic },
    /// A switch, reading 1 closed and 0 open. `between_readings` counts consecutive readings
    /// between the two, which make it a rheostat.
    Switch { between_readings: u32 },
    /// A rheostat, reading its resistance against its full scale.
    Rheostat,
}

/// When the ring is read to check it is still wired as when the element was identified: behind
/// a mono plug shorted to the sleeve, floating and reading like it, and behind a stereo plug
/// isolated, driven low with the sleeve and carrying no current. A stereo cable plugged into a
/// pedal while it is already in the jack can show a switch or rheostat between tip and sleeve
/// for a moment, before the ring connects.
#[derive(Debug, Clone, Copy)]
struct RingCheck {
    shorted: bool,
    periodic: Periodic,
    /// The last ring check disagreed. The element may just have switched between that check and
    /// the tip reading before it, so a fresh pair has to confirm it.
    mismatch: bool,
}

/// A check repeated while following.
#[derive(Debug, Clone, Copy)]
struct Periodic {
    interval: u64,
    due_at: u64,
}

impl Periodic {
    fn new(now: u64, interval: u64) -> Self {
        Self {
            interval,
            due_at: now + interval,
        }
    }

    /// Whether the check is due at `now`, scheduling the next one if it is.
    fn due(&mut self, now: u64) -> bool {
        if now < self.due_at {
            return false;
        }
        self.due_at = now + self.interval;
        true
    }
}

/// What has been learned about the pedal behind the plug, forgotten when the plug comes out or
/// nothing conducts behind it any more - a cable left in the jack while pedals are swapped at
/// its other end.
#[derive(Debug, Clone, Copy)]
struct Pedal {
    /// Total resistance of the first ring-sleeve end stop, in kΩ.
    first_end_stop_total: Option<f32>,
    /// Set once the element between tip and sleeve is known to be a rheostat: the largest
    /// full scale its readings called for, in kΩ, `0.0` before the first.
    rheostat_full_scale: Option<f32>,
}

impl Pedal {
    const UNKNOWN: Self = Self {
        first_end_stop_total: None,
        rheostat_full_scale: None,
    };
}

/// Follows one jack; see the module documentation.
pub struct JackMonitor {
    config: MonitorConfig,
    state: State,
    due_at: u64,
    rails: Option<Rails>,
    report: JackReport,
    /// Total resistance the floating taps last charged through, in kΩ.
    expected_total: f32,
    failed_solves: u32,
    pedal: Pedal,
    /// Wiper chosen in the settings, which decides an ambiguous end stop (ring and sleeve
    /// shorted: a ring wiper on the sleeve end, or a sleeve wiper on the ring end).
    preferred_wiper: Option<usize>,
    /// Wiper of the last potentiometer seen in this jack, the next choice at an end stop.
    remembered_wiper: Option<usize>,
}

impl JackMonitor {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            state: State::Rails {
                measurement: RailMeasurement::START,
                then: AfterRails::Empty,
            },
            due_at: 0,
            rails: None,
            report: JackReport::EMPTY,
            expected_total: f32::NAN,
            failed_solves: 0,
            pedal: Pedal::UNKNOWN,
            preferred_wiper: None,
            remembered_wiper: None,
        }
    }

    pub fn report(&self) -> &JackReport {
        &self.report
    }

    /// Time the next reading is due, in ms.
    pub fn due_at(&self) -> u64 {
        self.due_at
    }

    /// Sets which arm is the wiper when an end stop leaves it ambiguous, `None` to decide by
    /// what was seen before.
    pub fn set_preferred_wiper(&mut self, preferred_wiper: Option<usize>) {
        self.preferred_wiper = preferred_wiper;
    }

    /// The reading the monitor needs next, which stays the same until it is recorded.
    pub fn reading(&self) -> Reading {
        match &self.state {
            State::Rails { measurement, .. } => {
                let mut drives = [measurement.level; CONTACT_COUNT];
                drives[TIP_SWITCH] = Drive::Floating;
                Reading {
                    drives,
                    contact: measurement.arm,
                    settle_resistance: 0.0,
                }
            }

            State::Empty | State::ConfirmPlug { .. } => PLUG_CHECK,

            State::Solve { solve, .. } => {
                let (high, low) = solve.pair;
                Reading {
                    drives: pair_drives(high, low),
                    contact: pair_read_order(high, low)[solve.read],
                    settle_resistance: self.settle_resistance(),
                }
            }

            State::Follow(follower) if follower.next == TIP_SWITCH => PLUG_CHECK,

            State::Follow(follower) => Reading {
                drives: follower.drives(),
                contact: follower.next,
                settle_resistance: match follower.kind {
                    Followed::Potentiometer(_) => self.settle_resistance(),
                    // Only driven taps and a ring shorted to one of them are read.
                    Followed::TipSleeve(_) => 0.0,
                },
            },
        }
    }

    /// Takes the voltage of the reading [`reading`](Self::reading) handed out, read at `now`
    /// (in ms). Returns whether the report changed.
    pub fn record(&mut self, voltage: f32, now: u64) -> bool {
        let reading = self.reading();
        if reading.contact < ARM_COUNT {
            self.report.voltages[reading.contact] = voltage;
        } else {
            self.report.tip_switch_voltage = voltage;
        }
        self.report.drives = match &self.state {
            State::Follow(follower) => follower.drives(),
            _ => reading.drives,
        };

        let previous_mode = self.report.mode;
        let changed = match self.state {
            State::Rails { measurement, then } => self.record_rail(measurement, then, voltage, now),
            State::Empty => self.record_empty(voltage, now),
            State::Solve { solve, mode } => self.record_solve(solve, mode, voltage, now),
            State::ConfirmPlug { .. } => self.record_confirm_plug(voltage, now),
            State::Follow(follower) => self.record_follow(follower, voltage, now),
        };

        self.report.mode = self.state.mode();
        changed || self.report.mode != previous_mode
    }

    fn enter(&mut self, state: State, due_at: u64) {
        self.state = state;
        self.due_at = due_at;
    }

    fn settle_resistance(&self) -> f32 {
        if self.failed_solves > self.config.trusted_failed_solves {
            f32::NAN
        } else {
            self.expected_total
        }
    }

    fn record_rail(
        &mut self,
        mut measurement: RailMeasurement,
        then: AfterRails,
        voltage: f32,
        now: u64,
    ) -> bool {
        measurement.sum += voltage;
        measurement.sample += 1;
        if measurement.sample < self.config.rail_samples {
            self.enter(State::Rails { measurement, then }, now);
            return false;
        }

        let average = measurement.sum / measurement.sample as f32;
        match measurement.level {
            Drive::High => measurement.high[measurement.arm] = average,
            _ => measurement.low[measurement.arm] = average,
        }

        let next = match (measurement.level, measurement.arm + 1 < ARM_COUNT) {
            (level, true) => Some((level, measurement.arm + 1)),
            (Drive::High, false) => Some((Drive::Low, 0)),
            _ => None,
        };
        if let Some((level, arm)) = next {
            let measurement = RailMeasurement {
                level,
                arm,
                sample: 0,
                sum: 0.0,
                ..measurement
            };
            self.enter(State::Rails { measurement, then }, now);
            return false;
        }

        self.rails = Some(Rails {
            high: measurement.high,
            low: measurement.low,
            measured_at: now,
        });
        let next = match then {
            AfterRails::Empty => State::Empty,
            AfterRails::Solve(mode) => State::Solve {
                solve: Solve::new(self.config.solver),
                mode,
            },
        };
        self.enter(next, now);
        false
    }

    fn plugged(&self, tip_switch_voltage: f32) -> bool {
        let threshold = self.rails.as_ref().map_or(f32::NAN, |rails| {
            rails.low[TIP] + PLUG_THRESHOLD * (rails.high[TIP] - rails.low[TIP])
        });
        tip_switch_voltage > threshold
    }

    fn record_empty(&mut self, voltage: f32, now: u64) -> bool {
        if self.plugged(voltage) {
            self.identify(now);
        } else {
            self.empty(now);
        }
        false
    }

    fn record_confirm_plug(&mut self, voltage: f32, now: u64) -> bool {
        if !self.plugged(voltage) {
            self.empty(now);
            return false;
        }

        self.pedal = Pedal::UNKNOWN;
        let open = Element::Open {
            solve: Periodic::new(now, self.config.open_solve_interval),
        };
        self.follow_tip_sleeve(open, false, now);
        false
    }

    /// The plug is gone: forget everything learned about what was behind it, and poll for
    /// the next one.
    fn empty(&mut self, now: u64) {
        self.report = JackReport {
            mode: self.report.mode,
            voltages: self.report.voltages,
            drives: self.report.drives,
            tip_switch_voltage: self.report.tip_switch_voltage,
            ..JackReport::EMPTY
        };
        self.expected_total = f32::NAN;
        self.failed_solves = 0;
        self.pedal = Pedal::UNKNOWN;
        self.enter(State::Empty, now + self.config.plug_check_interval);
    }

    /// Something new is behind the plug: identify it, settling for the total a newly plugged
    /// network is assumed to have.
    fn identify(&mut self, now: u64) {
        self.expected_total = self.config.initial_total;
        self.start_solve(now, JackMode::Identifying);
    }

    /// The followed network no longer behaves like the one identified: unplugged, swapped or
    /// switched to another wiring. Identify it again.
    fn lose_track(&mut self, now: u64) {
        self.start_solve(now, JackMode::Identifying);
    }

    fn start_solve(&mut self, now: u64, mode: JackMode) {
        let stale = self.rails.as_ref().is_none_or(|rails| {
            now.saturating_sub(rails.measured_at) > self.config.rail_refresh_interval
        });

        let state = if stale {
            State::Rails {
                measurement: RailMeasurement::START,
                then: AfterRails::Solve(mode),
            }
        } else {
            State::Solve {
                solve: Solve::new(self.config.solver),
                mode,
            }
        };
        self.enter(state, now);
    }

    fn record_solve(&mut self, mut solve: Solve, mode: JackMode, voltage: f32, now: u64) -> bool {
        let (high, low) = solve.pair;
        let order = pair_read_order(high, low);
        solve.voltages[order[solve.read]] = voltage;
        solve.read += 1;
        if solve.read < order.len() {
            self.enter(State::Solve { solve, mode }, now);
            return false;
        }

        let Some(rails) = self.rails else {
            self.start_solve(now, mode);
            return false;
        };

        let measurement = PairMeasurement::from_voltages(
            PairVoltages {
                high: solve.voltages[high],
                low: solve.voltages[low],
                floating: solve.voltages[ARM_COUNT - high - low],
            },
            ArmDrive {
                rail_voltage: rails.high[high],
                pull_resistance: self.config.pull_up_resistances[high],
            },
            ArmDrive {
                rail_voltage: rails.low[low],
                pull_resistance: self.config.pull_down_resistances[low],
            },
            &self.config.solver,
        );

        let result = match measurement {
            Ok(measurement) => {
                solve.sequence.record(measurement);
                match solve.sequence.step() {
                    SolveStep::MeasurePair { high, low } => {
                        let solve = Solve::measuring(solve.sequence, high, low);
                        self.enter(State::Solve { solve, mode }, now);
                        return false;
                    }
                    SolveStep::Finished(result) => result,
                }
            }
            Err(error) => Err(error),
        };

        match result {
            Ok(resistances) => {
                self.failed_solves = 0;
                self.classify(resistances, mode, now);
                true
            }
            // Usually the pedal moving between pairs: solve again straight away, trusting
            // the last total for settling until it keeps failing.
            Err(_) => {
                self.failed_solves += 1;
                self.start_solve(now, mode);
                false
            }
        }
    }

    /// Decides what to do with the network a solve reported as `mode` found.
    fn classify(&mut self, resistances: ArmResistances, mode: JackMode, now: u64) {
        let network = resistances.network(
            self.config.max_wiper_relative,
            self.config.end_stop_relative,
        );
        self.expected_total = match network {
            Network::Disconnected => 0.0,
            _ => resistances.total,
        };
        self.report.resistances = resistances;

        match network {
            // Either the plug came out or nothing behind it conducts; only the tip switch
            // can tell.
            Network::Disconnected => {
                self.report.position = None;
                self.enter(State::ConfirmPlug { mode }, now);
            }

            Network::Potentiometer { wiper } => {
                self.remembered_wiper = Some(wiper);
                self.track(&resistances, wiper, now);
            }

            // Which of the two is the wiper only shows once the pedal leaves the stop. Track
            // the likelier one straight away: a wrong guess drives the true wiper as a track
            // end, so the end-to-end resistance the track ends see changes as soon as the
            // pedal moves, and the jack is identified again.
            Network::EndStop { candidates } => {
                if self.rheostat_behind_mono_plug(&resistances, candidates) {
                    self.pedal.rheostat_full_scale.get_or_insert(0.0);
                    self.follow_tip_sleeve(Element::Rheostat, true, now);
                    return;
                }

                let wiper = [
                    self.preferred_wiper,
                    self.remembered_wiper,
                    Some(TIP),
                    Some(RING),
                ]
                .into_iter()
                .flatten()
                .find(|wiper| candidates.contains(wiper))
                .unwrap_or(candidates[0]);
                self.track(&resistances, wiper, now);
            }

            Network::TipSleeve { ring_shorted } => {
                let element = match self.pedal.rheostat_full_scale {
                    Some(_) => Element::Rheostat,
                    None => Element::Switch {
                        between_readings: 0,
                    },
                };
                self.follow_tip_sleeve(element, ring_shorted, now);
            }

            Network::Other => {
                self.report.position = None;
                self.start_solve(now, JackMode::Other);
            }
        }
    }

    /// Whether an end stop with ring and sleeve shorted is really a rheostat between tip and
    /// sleeve behind a mono plug: the first such end stop since the plug went in is taken for
    /// a potentiometer, and only a total that has changed since then gives the rheostat away.
    fn rheostat_behind_mono_plug(
        &mut self,
        resistances: &ArmResistances,
        candidates: [usize; 2],
    ) -> bool {
        if !(candidates.contains(&RING) && candidates.contains(&SLEEVE)) {
            return false;
        }

        match self.pedal.first_end_stop_total {
            None => {
                self.pedal.first_end_stop_total = Some(resistances.total);
                false
            }
            Some(first) => {
                (resistances.total - first).abs() > self.config.end_stop_change_fraction * first
            }
        }
    }

    fn record_follow(&mut self, follower: Follower, voltage: f32, now: u64) -> bool {
        let Some(rails) = self.rails else {
            return false;
        };

        match follower.kind {
            Followed::Potentiometer(track) => {
                self.record_track(follower, track, voltage, &rails, now)
            }
            Followed::TipSleeve(tip_sleeve) => {
                self.record_tip_sleeve(follower, tip_sleeve, voltage, &rails, now)
            }
        }
    }

    /// Starts following a potentiometer with its wiper on `wiper`: its track ends read once
    /// each, then its wiper.
    fn track(&mut self, resistances: &ArmResistances, wiper: usize, now: u64) {
        let (low, high) = track_ends(wiper);

        // The drops the solve implies check the first reading of each end, so a pedal
        // unplugged right away is not taken for the reference.
        let (low_drop, high_drop) = match self.rails {
            Some(rails) => {
                let pull_up = self.config.pull_up_resistances[high];
                let pull_down = self.config.pull_down_resistances[low];
                let track = resistances.absolute(low) + resistances.absolute(high);
                let current = (rails.high[high] - rails.low[low]) / (pull_up + pull_down + track);
                (current * pull_down, current * pull_up)
            }
            None => (f32::NAN, f32::NAN),
        };
        let unread = |expected_drop: f32| TrackEnd {
            voltage: f32::NAN,
            expected_drop,
        };

        self.report.position = resistances.position_with_wiper(wiper);
        let track = Track {
            wiper,
            low_end: unread(low_drop),
            high_end: unread(high_drop),
            end_check: Periodic::new(now, self.config.end_tap_interval),
            next_end_is_high: false,
        };
        let follower = Follower {
            high,
            low,
            next: high,
            kind: Followed::Potentiometer(track),
        };
        self.enter(State::Follow(follower), now);
    }

    fn record_track(
        &mut self,
        mut follower: Follower,
        mut track: Track,
        voltage: f32,
        rails: &Rails,
        now: u64,
    ) -> bool {
        let read = follower.next;
        let consistent = if read == follower.high {
            let drop = rails.high[read] - voltage;
            self.end_consistent(&mut track.high_end, voltage, drop)
        } else if read == follower.low {
            let drop = voltage - rails.low[read];
            self.end_consistent(&mut track.low_end, voltage, drop)
        } else {
            self.wiper_in_range(&track, voltage)
        };

        if !consistent {
            self.lose_track(now);
            return true;
        }

        let wiper_read = read == track.wiper;
        if wiper_read {
            let span = track.high_end.voltage - track.low_end.voltage;
            let position = ((voltage - track.low_end.voltage) / span).clamp(0.0, 1.0);
            self.report.position = Some(position);
            self.follow_wiper(follower.low, follower.high, position);
        }

        // Both ends have to have been read once for the span the wiper divides.
        follower.next = if !track.high_end.read() {
            follower.high
        } else if !track.low_end.read() {
            follower.low
        } else if track.end_check.due(now) {
            track.next_end_is_high = !track.next_end_is_high;
            if track.next_end_is_high {
                follower.high
            } else {
                follower.low
            }
        } else {
            track.wiper
        };
        follower.kind = Followed::Potentiometer(track);
        self.enter(State::Follow(follower), now);

        wiper_read
    }

    /// Checks a track end's pull drop against the expected one: loosely against the solve's
    /// on the first reading, which then becomes the reference, and tightly from then on.
    fn end_consistent(&self, end: &mut TrackEnd, voltage: f32, drop: f32) -> bool {
        let fraction = if end.read() {
            self.config.end_drop_fraction
        } else {
            self.config.first_end_drop_fraction
        };
        let noise_tolerance = self.config.end_drop_sigmas * self.config.solver.voltage_noise;
        let tolerance = noise_tolerance.max(fraction * end.expected_drop.abs());

        // An end the solve could not imply a drop for (NaN) takes its first reading as is.
        if (drop - end.expected_drop).abs() > tolerance {
            return false;
        }

        end.expected_drop = if end.read() {
            // Average out the noise of the reading the reference started from.
            end.expected_drop + 0.1 * (drop - end.expected_drop)
        } else {
            drop
        };
        end.voltage = voltage;
        true
    }

    fn wiper_in_range(&self, track: &Track, voltage: f32) -> bool {
        let tolerance = self.config.wiper_range_sigmas * self.config.solver.voltage_noise;
        voltage >= track.low_end.voltage - tolerance
            && voltage <= track.high_end.voltage + tolerance
    }

    /// Moves the solved track halves to where the wiper now is, so the reported network
    /// follows the pedal.
    fn follow_wiper(&mut self, low_end: usize, high_end: usize, position: f32) {
        let relative = &mut self.report.resistances.relative;
        let track = relative[low_end] + relative[high_end];
        relative[low_end] = position * track;
        relative[high_end] = (1.0 - position) * track;
    }

    /// Starts following `element` between tip and sleeve, with the ring shorted to the sleeve
    /// behind a mono plug and isolated behind a stereo one.
    fn follow_tip_sleeve(&mut self, element: Element, ring_shorted: bool, now: u64) {
        let ring_check = RingCheck {
            shorted: ring_shorted,
            periodic: Periodic::new(now, self.config.ring_check_interval),
            mismatch: false,
        };
        let tip_sleeve = TipSleeve {
            element,
            plug_check: Periodic::new(now, self.config.plug_check_interval),
            ring_check,
            sleeve_voltage: f32::NAN,
        };
        let follower = Follower {
            high: TIP,
            low: SLEEVE,
            next: TIP,
            kind: Followed::TipSleeve(tip_sleeve),
        };
        self.enter(State::Follow(follower), now);
    }

    fn record_tip_sleeve(
        &mut self,
        mut follower: Follower,
        mut tip_sleeve: TipSleeve,
        voltage: f32,
        rails: &Rails,
        now: u64,
    ) -> bool {
        let read = follower.next;
        let mut changed = false;

        if read == TIP_SWITCH {
            if !self.plugged(voltage) {
                self.empty(now);
                return false;
            }
        } else if read == RING {
            let ring_check = &mut tip_sleeve.ring_check;
            let mismatch = if ring_check.shorted {
                self.ring_left_sleeve(voltage, tip_sleeve.sleeve_voltage, rails)
            } else {
                self.ring_connected(voltage, rails)
            };
            // Whatever is behind the plug now is not what was identified.
            if mismatch && ring_check.mismatch {
                self.identify(now);
                return true;
            }
            ring_check.mismatch = mismatch;
        } else {
            let (resistance, sleeve_voltage) = self.tip_sleeve_resistance(voltage, rails);
            tip_sleeve.sleeve_voltage = sleeve_voltage;

            if let Element::Open { .. } = tip_sleeve.element {
                // Something connects tip and sleeve now: find out what.
                if resistance.is_finite() {
                    self.identify(now);
                    return true;
                }
            } else {
                self.report.position = self.element_position(&mut tip_sleeve.element, resistance);
                self.report.resistances =
                    element_resistances(tip_sleeve.ring_check.shorted, resistance);
                changed = true;
            }
        }

        if let Element::Open { solve } = &mut tip_sleeve.element
            && solve.due(now)
        {
            self.start_solve(now, JackMode::Open);
            return changed;
        }

        follower.next = if tip_sleeve.plug_check.due(now) {
            TIP_SWITCH
        } else if (tip_sleeve.ring_check.mismatch && read == TIP)
            || tip_sleeve.ring_check.periodic.due(now)
        {
            // A mismatch is confirmed straight after the fresh tip reading.
            RING
        } else {
            TIP
        };
        follower.kind = Followed::TipSleeve(tip_sleeve);
        self.enter(State::Follow(follower), now);

        changed
    }

    /// Whether a mono plug's ring reads away from the sleeve voltage the last tip reading
    /// implied.
    fn ring_left_sleeve(&self, ring_voltage: f32, sleeve_voltage: f32, rails: &Rails) -> bool {
        let sleeve_drop = (sleeve_voltage - rails.low[SLEEVE]).abs();
        let tolerance = self.config.wiper_range_sigmas * self.config.solver.voltage_noise
            + self.config.ring_check_fraction * sleeve_drop;
        sleeve_voltage.is_finite() && (ring_voltage - sleeve_voltage).abs() > tolerance
    }

    /// Whether a stereo plug's ring, driven low, carries current, by the same measure that tells
    /// tip and sleeve connected, so noise never trips it.
    fn ring_connected(&self, ring_voltage: f32, rails: &Rails) -> bool {
        let drop = ring_voltage - rails.low[RING];
        drop > self.config.element_open_sigmas * self.config.solver.voltage_noise
    }

    /// Resistance between tip and sleeve while the tip is driven high and the sleeve low, from
    /// the tip's reading alone, and the voltage the sleeve's tap is at meanwhile. Both pulls
    /// carry the same current, so the sleeve's drop follows from the tip's.
    fn tip_sleeve_resistance(&self, tip_voltage: f32, rails: &Rails) -> (f32, f32) {
        let pull_up = self.config.pull_up_resistances[TIP];
        let pull_down = self.config.pull_down_resistances[SLEEVE];
        let current = (rails.high[TIP] - tip_voltage) / pull_up;
        let current_noise = self.config.solver.voltage_noise / pull_up;

        if current <= self.config.element_open_sigmas * current_noise {
            return (f32::INFINITY, rails.low[SLEEVE]);
        }

        let sleeve_voltage = rails.low[SLEEVE] + current * pull_down;
        let resistance = (tip_voltage - sleeve_voltage).max(0.0) / current;
        (resistance, sleeve_voltage)
    }

    /// The position a switch or rheostat `element` of `resistance` reads as: a switch is 1
    /// closed and 0 open, a rheostat its resistance against its full scale. A switch keeps its
    /// last position through readings between closed and open, until enough of them in a row
    /// make it a rheostat.
    fn element_position(&mut self, element: &mut Element, resistance: f32) -> Option<f32> {
        let closed = resistance <= self.config.closed_resistance;
        let between = resistance.is_finite() && !closed;

        if let Element::Switch { between_readings } = element {
            *between_readings = if between { *between_readings + 1 } else { 0 };
            if *between_readings >= self.config.rheostat_readings {
                *element = Element::Rheostat;
                self.pedal.rheostat_full_scale.get_or_insert(0.0);
            }
        }

        match element {
            Element::Rheostat if !resistance.is_finite() => Some(1.0),
            Element::Rheostat => {
                let full_scale = self
                    .pedal
                    .rheostat_full_scale
                    .unwrap_or(0.0)
                    .max(full_scale(resistance));
                self.pedal.rheostat_full_scale = Some(full_scale);
                Some((resistance / full_scale).clamp(0.0, 1.0))
            }
            _ if between => self.report.position,
            _ => Some(if closed { 1.0 } else { 0.0 }),
        }
    }
}

/// Drives `high` high and `low` low, leaving every other contact floating.
fn pair_drives(high: usize, low: usize) -> [Drive; CONTACT_COUNT] {
    let mut drives = [Drive::Floating; CONTACT_COUNT];
    drives[high] = Drive::High;
    drives[low] = Drive::Low;
    drives
}

/// The arm resistances a tip-sleeve element of `resistance` kΩ shows as, for the report: the
/// tip's arm carries it and the star point sits on the sleeve, with the ring on the star point
/// behind a mono plug and isolated behind a stereo one.
fn element_resistances(ring_shorted: bool, resistance: f32) -> ArmResistances {
    let open = !resistance.is_finite();
    let shorted = resistance == 0.0;
    let relative = match (ring_shorted, open, shorted) {
        (true, true, _) => [f32::INFINITY, 0.0, 0.0],
        (true, false, true) => [0.0, 0.0, 0.0],
        (true, false, false) => [1.0, 0.0, 0.0],
        (false, true, _) => [f32::INFINITY; ARM_COUNT],
        (false, false, true) => [0.0, f32::INFINITY, 0.0],
        (false, false, false) => [1.0, f32::INFINITY, 0.0],
    };

    ArmResistances {
        relative,
        total: if open { f32::NAN } else { resistance },
    }
}

/// Full scale of a rheostat measuring `resistance` kΩ: the smallest standard value it could
/// be within its tolerance.
fn full_scale(resistance: f32) -> f32 {
    RHEOSTAT_VALUES
        .into_iter()
        .find(|&value| value * RHEOSTAT_TOLERANCE >= resistance)
        .unwrap_or(resistance)
}

/// Order the taps of a pair are read in: the two driven ones first, and the floating one
/// last, since it charges its filter capacitor through the whole network.
fn pair_read_order(high: usize, low: usize) -> [usize; ARM_COUNT] {
    [high, low, ARM_COUNT - high - low]
}

impl core::fmt::Debug for JackMonitor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("JackMonitor")
            .field("state", &self.state)
            .field("report", &self.report)
            .field("due_at", &self.due_at)
            .finish_non_exhaustive()
    }
}
