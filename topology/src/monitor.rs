//! Watches one jack over time: notices a plug going in or out, identifies what is plugged in
//! with full solves, and then follows a potentiometer's wiper with single readings.
//!
//! [`JackMonitor`] hands out one ADC reading at a time and takes its voltage back, so a
//! scheduler can interleave jacks on different ADCs, and tests can drive it from a model.

use crate::config::SolverConfig;
use crate::measurement::{ArmDrive, PairMeasurement, PairVoltages};
use crate::resistances::{ARM_COUNT, ArmResistances, Potentiometer, track_ends};
use crate::sequence::{SolveSequence, SolveStep};

/// Contacts of one jack that can be driven and read: its three arms plus the tip's
/// normalling contact.
pub const CONTACT_COUNT: usize = ARM_COUNT + 1;

/// Index of the tip switch contact among a jack's contacts.
pub const TIP_SWITCH: usize = ARM_COUNT;

/// Arm the tip switch touches while no plug is inserted.
pub const TIP: usize = 0;

/// Arm of the ring, the wiper of the one common wiring without it on the tip.
const RING: usize = 1;

/// Arm of the sleeve, the ground end of every common wiring.
const SLEEVE: usize = 2;

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

/// What a monitor currently makes of its jack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum JackMode {
    /// No plug in the jack.
    Empty,
    /// Full solves until the network is known: just plugged in, or tracking lost it.
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
    /// Wiper position in `0.0..=1.0`, as [`ArmResistances::wiper_position`] defines it.
    pub position: Option<f32>,
    pub resistances: ArmResistances,
    /// Last voltage read at each arm's tap, in V.
    pub voltages: [f32; ARM_COUNT],
    /// How each contact (the arms, then the tip switch) is driven for the reading the monitor
    /// is on: that of its last reading, except for the plug checks interleaved with following
    /// a switch or rheostat, which briefly drive the tip low and the tip switch high.
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

    /// Largest relative resistance that still counts as a wiper at the star point.
    pub max_wiper_relative: f32,

    /// How often an empty jack is checked for a plug, and a jack followed through its tip
    /// for the plug still being there.
    pub empty_poll_interval: u64,
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
            max_wiper_relative: ArmResistances::DEFAULT_MAX_WIPER_RELATIVE,
            empty_poll_interval: 50,
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

#[derive(Debug, Clone, Copy)]
enum Task {
    /// Averaging every arm's tap with all arms driven to `level`, high first.
    Rails {
        level: Drive,
        arm: usize,
        sample: u32,
        sum: f32,
        high: [f32; ARM_COUNT],
        low: [f32; ARM_COUNT],
    },
    /// The tip pulled low and the tip switch high, reading the tip switch.
    PlugCheck,
    /// A full solve, reading the three taps of each pair in turn.
    Solve {
        sequence: SolveSequence,
        pair: (usize, usize),
        voltages: [f32; ARM_COUNT],
        read: usize,
    },
    Track(Tracking),
    TwoTerminal(TwoTerminal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementRead {
    Tip,
    Ring,
    PlugCheck,
}

/// Following whatever connects tip and sleeve: the tip driven high, the sleeve low, and the
/// resistance between them read off the tip's pull drop.
#[derive(Debug, Clone, Copy)]
struct TwoTerminal {
    /// A mono plug shorts the ring to the sleeve, which every ring check confirms.
    ring_shorted: bool,
    /// Nothing is known to connect: the first conduction sends the jack to be identified.
    open: bool,
    next: ElementRead,
    next_plug_check_at: u64,
    next_ring_check_at: u64,
    next_solve_at: u64,
    /// Voltage the sleeve's tap is at, from the last tip reading.
    sleeve_voltage: f32,
    /// The last ring check disagreed with the sleeve. The element may just have switched
    /// between that check and the tip reading before it, so a fresh pair has to confirm it.
    ring_mismatch: bool,
}

/// What has been learned about the element between tip and sleeve since the plug went in.
#[derive(Debug, Clone, Copy)]
struct Element {
    rheostat: bool,
    /// Consecutive readings between closed and open.
    between_readings: u32,
    /// A rheostat's full scale, in kΩ.
    full_scale: f32,
}

impl Element {
    const UNKNOWN: Self = Self {
        rheostat: false,
        between_readings: 0,
        full_scale: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackRead {
    Wiper,
    LowEnd,
    HighEnd,
}

#[derive(Debug, Clone, Copy)]
struct Tracking {
    wiper: usize,
    low_end: usize,
    high_end: usize,
    next: TrackRead,
    low_voltage: f32,
    high_voltage: f32,
    low_drop: EndDrop,
    high_drop: EndDrop,
    next_end_read_at: u64,
    next_end_is_high: bool,
}

/// Pull drop a tracked track end is expected to show, in V: implied by the solve (`NaN` if it
/// could not), then taken from the first reading and refined by every one that agrees.
#[derive(Debug, Clone, Copy)]
struct EndDrop {
    expected: f32,
    read: bool,
}

/// Follows one jack; see the module documentation.
pub struct JackMonitor {
    config: MonitorConfig,
    task: Task,
    due_at: u64,
    rails: Option<Rails>,
    report: JackReport,
    /// Total resistance the floating taps last charged through, in kΩ.
    expected_total: f32,
    failed_solves: u32,
    /// Wiper chosen in the settings, which decides an ambiguous end stop (ring and sleeve
    /// shorted: a ring wiper on the sleeve end, or a sleeve wiper on the ring end).
    preferred_wiper: Option<usize>,
    /// Wiper of the last potentiometer seen in this jack, the next choice at an end stop.
    remembered_wiper: Option<usize>,
    element: Element,
    /// Total resistance of the first ring-sleeve end stop since the plug went in, in kΩ.
    first_end_stop_total: f32,
}

impl JackMonitor {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            task: Self::rails_task(),
            due_at: 0,
            rails: None,
            report: JackReport::EMPTY,
            expected_total: f32::NAN,
            failed_solves: 0,
            preferred_wiper: None,
            remembered_wiper: None,
            element: Element::UNKNOWN,
            first_end_stop_total: f32::NAN,
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
        let floating = [Drive::Floating; CONTACT_COUNT];

        match &self.task {
            Task::Rails { level, arm, .. } => {
                let mut drives = [*level; CONTACT_COUNT];
                drives[TIP_SWITCH] = Drive::Floating;
                Reading {
                    drives,
                    contact: *arm,
                    settle_resistance: 0.0,
                }
            }

            Task::PlugCheck => {
                let mut drives = floating;
                drives[TIP] = Drive::Low;
                drives[TIP_SWITCH] = Drive::High;
                Reading {
                    drives,
                    contact: TIP_SWITCH,
                    settle_resistance: 0.0,
                }
            }

            Task::Solve {
                pair: (high, low),
                read,
                ..
            } => {
                let mut drives = floating;
                drives[*high] = Drive::High;
                drives[*low] = Drive::Low;
                Reading {
                    drives,
                    contact: pair_read_order(*high, *low)[*read],
                    settle_resistance: self.settle_resistance(),
                }
            }

            Task::TwoTerminal(element) => {
                let mut drives = floating;
                let contact = match element.next {
                    ElementRead::PlugCheck => {
                        drives[TIP] = Drive::Low;
                        drives[TIP_SWITCH] = Drive::High;
                        TIP_SWITCH
                    }
                    ElementRead::Tip | ElementRead::Ring => {
                        drives[TIP] = Drive::High;
                        drives[SLEEVE] = Drive::Low;
                        if element.next == ElementRead::Ring {
                            RING
                        } else {
                            TIP
                        }
                    }
                };
                // Only driven taps and a ring shorted to one of them are read.
                Reading {
                    drives,
                    contact,
                    settle_resistance: 0.0,
                }
            }

            Task::Track(tracking) => {
                let mut drives = floating;
                drives[tracking.low_end] = Drive::Low;
                drives[tracking.high_end] = Drive::High;
                let contact = match tracking.next {
                    TrackRead::Wiper => tracking.wiper,
                    TrackRead::LowEnd => tracking.low_end,
                    TrackRead::HighEnd => tracking.high_end,
                };
                Reading {
                    drives,
                    contact,
                    settle_resistance: self.settle_resistance(),
                }
            }
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
        let interleaved_plug_check = matches!(
            &self.task,
            Task::TwoTerminal(element) if element.next == ElementRead::PlugCheck
        );
        if !interleaved_plug_check {
            self.report.drives = reading.drives;
        }

        match self.task {
            Task::Rails { .. } => self.record_rail(voltage, now),
            Task::PlugCheck => self.record_plug_check(voltage, now),
            Task::Solve { .. } => self.record_solve(voltage, now),
            Task::Track(_) => self.record_track(voltage, now),
            Task::TwoTerminal(_) => self.record_two_terminal(voltage, now),
        }
    }

    fn settle_resistance(&self) -> f32 {
        if self.failed_solves > self.config.trusted_failed_solves {
            f32::NAN
        } else {
            self.expected_total
        }
    }

    fn rails_task() -> Task {
        Task::Rails {
            level: Drive::High,
            arm: 0,
            sample: 0,
            sum: 0.0,
            high: [f32::NAN; ARM_COUNT],
            low: [f32::NAN; ARM_COUNT],
        }
    }

    fn record_rail(&mut self, voltage: f32, now: u64) -> bool {
        let Task::Rails {
            level,
            arm,
            sample,
            sum,
            mut high,
            mut low,
        } = self.task
        else {
            return false;
        };

        let sum = sum + voltage;
        let sample = sample + 1;
        if sample < self.config.rail_samples {
            self.task = Task::Rails {
                level,
                arm,
                sample,
                sum,
                high,
                low,
            };
            return false;
        }

        let average = sum / sample as f32;
        match level {
            Drive::High => high[arm] = average,
            _ => low[arm] = average,
        }

        let next = match (level, arm + 1 < ARM_COUNT) {
            (_, true) => Some((level, arm + 1)),
            (Drive::High, false) => Some((Drive::Low, 0)),
            _ => None,
        };

        if let Some((level, arm)) = next {
            self.task = Task::Rails {
                level,
                arm,
                sample: 0,
                sum: 0.0,
                high,
                low,
            };
            return false;
        }

        self.rails = Some(Rails {
            high,
            low,
            measured_at: now,
        });

        // Rails are measured on start-up, where the plug check comes next, or ahead of a
        // solve.
        if self.report.mode == JackMode::Empty {
            self.task = Task::PlugCheck;
        } else {
            self.task = self.solve_task();
        }
        self.due_at = now;
        false
    }

    /// Whether the tip switch's reading during a plug check shows a plug: without one it
    /// touches the pulled-down tip and the two pulls divide the rails in half, with one it is
    /// isolated and reads its own rail.
    fn plugged(&self, tip_switch_voltage: f32) -> bool {
        let threshold = self.rails.as_ref().map_or(f32::NAN, |rails| {
            rails.low[TIP] + 0.75 * (rails.high[TIP] - rails.low[TIP])
        });
        tip_switch_voltage > threshold
    }

    fn record_plug_check(&mut self, voltage: f32, now: u64) -> bool {
        if !self.plugged(voltage) {
            return self.empty(now);
        }

        match self.report.mode {
            JackMode::Empty => {
                self.report.mode = JackMode::Identifying;
                self.expected_total = self.config.initial_total;
                self.start_solve(now);
                true
            }
            // Nothing conducted behind the plug: watch tip and sleeve for connecting.
            mode => {
                self.report.mode = JackMode::Open;
                self.report.position = None;
                self.start_two_terminal(false, true, now);
                mode != JackMode::Open
            }
        }
    }

    /// The plug is gone: forget everything learned about what was behind it, and poll for
    /// the next one.
    fn empty(&mut self, now: u64) -> bool {
        let changed = self.report.mode != JackMode::Empty;
        self.report = JackReport {
            voltages: self.report.voltages,
            drives: self.report.drives,
            tip_switch_voltage: self.report.tip_switch_voltage,
            ..JackReport::EMPTY
        };
        self.expected_total = 0.0;
        self.failed_solves = 0;
        self.element = Element::UNKNOWN;
        self.first_end_stop_total = f32::NAN;
        self.task = Task::PlugCheck;
        self.due_at = now + self.config.empty_poll_interval;
        changed
    }

    fn start_solve(&mut self, due_at: u64) {
        let stale = self.rails.as_ref().is_none_or(|rails| {
            due_at.saturating_sub(rails.measured_at) > self.config.rail_refresh_interval
        });

        self.task = if stale {
            Self::rails_task()
        } else {
            self.solve_task()
        };
        self.due_at = due_at;
    }

    fn solve_task(&self) -> Task {
        let sequence = SolveSequence::new(self.config.solver);
        let pair = match sequence.step() {
            SolveStep::MeasurePair { high, low } => (high, low),
            // A fresh sequence always starts with a pair.
            SolveStep::Finished(_) => (0, 1),
        };

        Task::Solve {
            sequence,
            pair,
            voltages: [f32::NAN; ARM_COUNT],
            read: 0,
        }
    }

    fn record_solve(&mut self, voltage: f32, now: u64) -> bool {
        let Task::Solve {
            mut sequence,
            pair: (high, low),
            mut voltages,
            read,
        } = self.task
        else {
            return false;
        };

        let order = pair_read_order(high, low);
        voltages[order[read]] = voltage;
        if read + 1 < order.len() {
            self.task = Task::Solve {
                sequence,
                pair: (high, low),
                voltages,
                read: read + 1,
            };
            self.due_at = now;
            return false;
        }

        let Some(rails) = self.rails else {
            self.start_solve(now);
            return false;
        };

        let measurement = PairMeasurement::from_voltages(
            PairVoltages {
                high: voltages[high],
                low: voltages[low],
                floating: voltages[ARM_COUNT - high - low],
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
                sequence.record(measurement);
                match sequence.step() {
                    SolveStep::MeasurePair { high, low } => {
                        self.task = Task::Solve {
                            sequence,
                            pair: (high, low),
                            voltages: [f32::NAN; ARM_COUNT],
                            read: 0,
                        };
                        self.due_at = now;
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
                self.classify(resistances, now)
            }
            // Usually the pedal moving between pairs: solve again straight away, trusting
            // the last total for settling until it keeps failing.
            Err(_) => {
                self.failed_solves += 1;
                self.start_solve(now);
                false
            }
        }
    }

    fn classify(&mut self, resistances: ArmResistances, now: u64) -> bool {
        let disconnected = resistances
            .relative
            .iter()
            .all(|relative| relative.is_infinite());
        self.expected_total = if disconnected { 0.0 } else { resistances.total };
        self.report.resistances = resistances;

        if disconnected {
            // Either the plug came out or nothing behind it conducts; only the tip switch
            // can tell.
            self.report.position = None;
            self.task = Task::PlugCheck;
            self.due_at = now;
            return true;
        }

        match resistances.potentiometer(self.config.max_wiper_relative) {
            Some(Potentiometer::Wiper(wiper)) => {
                self.remembered_wiper = Some(wiper);
                self.report.mode = JackMode::Tracking;
                self.report.position = resistances.position_with_wiper(wiper);
                self.start_tracking(&resistances, wiper, now);
            }

            // Which of the two is the wiper only shows once the pedal leaves the stop. Track
            // the likelier one straight away: a wrong guess drives the true wiper as a track
            // end, so the end-to-end resistance the track ends see changes as soon as the
            // pedal moves, and the jack is identified again.
            Some(Potentiometer::EndStop(candidates)) => {
                if self.rheostat_behind_mono_plug(&resistances, candidates) {
                    self.element.rheostat = true;
                    self.report.mode = JackMode::Rheostat;
                    self.start_two_terminal(true, false, now);
                    return true;
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
                self.report.mode = JackMode::Tracking;
                self.report.position = resistances.position_with_wiper(wiper);
                self.start_tracking(&resistances, wiper, now);
            }

            None => match tip_sleeve_element(&resistances) {
                Some(ring_shorted) => {
                    self.report.mode = if self.element.rheostat {
                        JackMode::Rheostat
                    } else {
                        JackMode::Switch
                    };
                    self.start_two_terminal(ring_shorted, false, now);
                }
                None => {
                    self.report.mode = JackMode::Other;
                    self.report.position = None;
                    self.start_solve(now);
                }
            },
        }

        true
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

        let first = self.first_end_stop_total;
        if !first.is_finite() {
            self.first_end_stop_total = resistances.total;
            return false;
        }

        (resistances.total - first).abs() > self.config.end_stop_change_fraction * first
    }

    fn start_two_terminal(&mut self, ring_shorted: bool, open: bool, now: u64) {
        self.task = Task::TwoTerminal(TwoTerminal {
            ring_shorted,
            open,
            next: ElementRead::Tip,
            next_plug_check_at: now + self.config.empty_poll_interval,
            next_ring_check_at: now + self.config.ring_check_interval,
            next_solve_at: now + self.config.open_solve_interval,
            sleeve_voltage: f32::NAN,
            ring_mismatch: false,
        });
        self.due_at = now;
    }

    fn record_two_terminal(&mut self, voltage: f32, now: u64) -> bool {
        let Task::TwoTerminal(mut element) = self.task else {
            return false;
        };
        let Some(rails) = self.rails else {
            return false;
        };
        self.due_at = now;
        let mut changed = false;

        match element.next {
            ElementRead::PlugCheck => {
                if !self.plugged(voltage) {
                    return self.empty(now);
                }
            }

            ElementRead::Ring => {
                let sleeve_drop = (element.sleeve_voltage - rails.low[SLEEVE]).abs();
                let tolerance = self.config.wiper_range_sigmas * self.config.solver.voltage_noise
                    + self.config.ring_check_fraction * sleeve_drop;
                let still_shorted = (voltage - element.sleeve_voltage).abs() <= tolerance;
                let mismatch =
                    element.ring_shorted && element.sleeve_voltage.is_finite() && !still_shorted;
                if mismatch && element.ring_mismatch {
                    self.lose_track(now);
                    return true;
                }
                element.ring_mismatch = mismatch;
            }

            ElementRead::Tip => {
                let (resistance, sleeve_voltage) = self.tip_sleeve_resistance(voltage, &rails);
                element.sleeve_voltage = sleeve_voltage;

                if element.open {
                    // Something connects tip and sleeve now: find out what.
                    if resistance.is_finite() {
                        self.report.mode = JackMode::Identifying;
                        self.expected_total = self.config.initial_total;
                        self.start_solve(now);
                        return true;
                    }
                } else {
                    let position = self.element_position(resistance);
                    self.report.position = position;
                    self.report.mode = if self.element.rheostat {
                        JackMode::Rheostat
                    } else {
                        JackMode::Switch
                    };
                    self.report.resistances = element_resistances(element.ring_shorted, resistance);
                    changed = true;
                }
            }
        }

        if element.open && now >= element.next_solve_at {
            self.start_solve(now);
            return changed;
        }

        element.next = if now >= element.next_plug_check_at {
            element.next_plug_check_at = now + self.config.empty_poll_interval;
            ElementRead::PlugCheck
        } else if element.ring_mismatch && element.next == ElementRead::Tip {
            // Straight after the fresh tip reading.
            ElementRead::Ring
        } else if element.ring_shorted && now >= element.next_ring_check_at {
            element.next_ring_check_at = now + self.config.ring_check_interval;
            ElementRead::Ring
        } else {
            ElementRead::Tip
        };
        self.task = Task::TwoTerminal(element);

        changed
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

    /// The position a tip-sleeve element's `resistance` reads as: a switch is 1 closed and 0
    /// open, a rheostat its resistance against its full scale. Readings between closed and
    /// open keep the last position until enough of them in a row make it a rheostat.
    fn element_position(&mut self, resistance: f32) -> Option<f32> {
        let closed = resistance <= self.config.closed_resistance;
        let between = resistance.is_finite() && !closed;

        self.element.between_readings = if between {
            self.element.between_readings + 1
        } else {
            0
        };
        if self.element.between_readings >= self.config.rheostat_readings {
            self.element.rheostat = true;
        }

        if self.element.rheostat {
            if !resistance.is_finite() {
                return Some(1.0);
            }
            self.element.full_scale = self.element.full_scale.max(full_scale(resistance));
            return Some((resistance / self.element.full_scale).clamp(0.0, 1.0));
        }

        if between {
            return self.report.position;
        }

        Some(if closed { 1.0 } else { 0.0 })
    }

    fn start_tracking(&mut self, resistances: &ArmResistances, wiper: usize, now: u64) {
        let (low_end, high_end) = track_ends(wiper);

        // The drops the solve implies check the first reading of each end, so a pedal
        // unplugged right away is not taken for the reference.
        let (low_drop, high_drop) = match self.rails {
            Some(rails) => {
                let pull_up = self.config.pull_up_resistances[high_end];
                let pull_down = self.config.pull_down_resistances[low_end];
                let track = resistances.absolute(low_end) + resistances.absolute(high_end);
                let current =
                    (rails.high[high_end] - rails.low[low_end]) / (pull_up + pull_down + track);
                (current * pull_down, current * pull_up)
            }
            None => (f32::NAN, f32::NAN),
        };
        let expect = |expected: f32| EndDrop {
            expected,
            read: false,
        };

        self.task = Task::Track(Tracking {
            wiper,
            low_end,
            high_end,
            next: TrackRead::HighEnd,
            low_voltage: f32::NAN,
            high_voltage: f32::NAN,
            low_drop: expect(low_drop),
            high_drop: expect(high_drop),
            next_end_read_at: now + self.config.end_tap_interval,
            next_end_is_high: false,
        });
        self.due_at = now;
    }

    fn record_track(&mut self, voltage: f32, now: u64) -> bool {
        let Task::Track(mut tracking) = self.task else {
            return false;
        };
        let Some(rails) = self.rails else {
            return false;
        };
        self.due_at = now;

        let consistent = match tracking.next {
            TrackRead::HighEnd => {
                tracking.high_voltage = voltage;
                let drop = rails.high[tracking.high_end] - voltage;
                self.end_drop_consistent(&mut tracking.high_drop, drop)
            }
            TrackRead::LowEnd => {
                tracking.low_voltage = voltage;
                let drop = voltage - rails.low[tracking.low_end];
                self.end_drop_consistent(&mut tracking.low_drop, drop)
            }
            TrackRead::Wiper => self.wiper_in_range(&tracking, voltage),
        };

        if !consistent {
            self.lose_track(now);
            return true;
        }

        let changed = tracking.next == TrackRead::Wiper;
        if changed {
            let span = tracking.high_voltage - tracking.low_voltage;
            let position = ((voltage - tracking.low_voltage) / span).clamp(0.0, 1.0);
            self.report.position = Some(position);
            self.follow_wiper(&tracking, position);
        }

        // Both ends have to have been read once for the span the wiper divides.
        tracking.next = match (
            tracking.low_voltage.is_nan(),
            tracking.high_voltage.is_nan(),
        ) {
            (_, true) => TrackRead::HighEnd,
            (true, _) => TrackRead::LowEnd,
            _ if now >= tracking.next_end_read_at => {
                tracking.next_end_read_at = now + self.config.end_tap_interval;
                tracking.next_end_is_high = !tracking.next_end_is_high;
                if tracking.next_end_is_high {
                    TrackRead::HighEnd
                } else {
                    TrackRead::LowEnd
                }
            }
            _ => TrackRead::Wiper,
        };
        self.task = Task::Track(tracking);

        changed
    }

    /// Checks a track end's pull drop against the expected one: loosely against the solve's
    /// on the first reading, which then becomes the reference, and tightly from then on.
    fn end_drop_consistent(&self, reference: &mut EndDrop, drop: f32) -> bool {
        let fraction = if reference.read {
            self.config.end_drop_fraction
        } else {
            self.config.first_end_drop_fraction
        };
        let noise_tolerance = self.config.end_drop_sigmas * self.config.solver.voltage_noise;
        let tolerance = noise_tolerance.max(fraction * reference.expected.abs());

        // An end the solve could not imply a drop for (NaN) takes its first reading as is.
        if (drop - reference.expected).abs() > tolerance {
            return false;
        }

        reference.expected = if reference.read {
            // Average out the noise of the reading the reference started from.
            reference.expected + 0.1 * (drop - reference.expected)
        } else {
            drop
        };
        reference.read = true;
        true
    }

    fn wiper_in_range(&self, tracking: &Tracking, voltage: f32) -> bool {
        let tolerance = self.config.wiper_range_sigmas * self.config.solver.voltage_noise;
        voltage >= tracking.low_voltage - tolerance && voltage <= tracking.high_voltage + tolerance
    }

    /// Moves the solved track halves to where the wiper now is, so the reported network
    /// follows the pedal.
    fn follow_wiper(&mut self, tracking: &Tracking, position: f32) {
        let relative = &mut self.report.resistances.relative;
        let track = relative[tracking.low_end] + relative[tracking.high_end];
        relative[tracking.low_end] = position * track;
        relative[tracking.high_end] = (1.0 - position) * track;
    }

    /// The tracked network no longer behaves like the one identified: unplugged, swapped or
    /// switched to another wiring. Identify it again.
    fn lose_track(&mut self, now: u64) {
        self.report.mode = JackMode::Identifying;
        self.start_solve(now);
    }
}

/// The tip-sleeve element a solved network holds, if that is all it holds: `Some(true)` behind
/// a mono plug (ring shorted to the sleeve, whatever the tip does), `Some(false)` behind a
/// stereo plug with the ring unconnected.
fn tip_sleeve_element(resistances: &ArmResistances) -> Option<bool> {
    let [tip, ring, sleeve] = resistances.relative;
    let at_star_point = |relative: f32| relative <= ArmResistances::END_STOP_RELATIVE;

    if at_star_point(ring) && at_star_point(sleeve) {
        Some(true)
    } else if ring.is_infinite() && !(tip.is_infinite() && sleeve.is_infinite()) {
        Some(false)
    } else {
        None
    }
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
            .field("report", &self.report)
            .field("due_at", &self.due_at)
            .finish_non_exhaustive()
    }
}
