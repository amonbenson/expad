use crate::config::SolverConfig;
use crate::measurement::PairMeasurement;
use crate::resistances::{ARM_COUNT, ArmResistances};
use crate::resolve::{self, DrivenPair};

/// The pairs a solve can ask for, as `(high, low)` arm indices, in the order they are
/// measured. The first arm is driven high in both of the first two, so the network only
/// has to be reconfigured on the low side between them. The third pair is only measured
/// when the first two cannot settle the network on their own: when neither conducts, or
/// when the first arm is so close to 0 kΩ that their voltage ratios say nothing about how
/// the other two arms compare.
pub const PAIR_SEQUENCE: [(usize, usize); ARM_COUNT] = [(0, 1), (0, 2), (1, 2)];

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SolveError {
    /// The high and low side of one loop reported different currents, so at least one of
    /// the two readings cannot be trusted.
    CurrentConsistency { high_current: f32, low_current: f32 },

    /// The pairs imply different total resistances, beyond what the noise on their currents
    /// explains, so they cannot all describe the same network.
    ResistanceConsistency { total: f32, disagreeing_total: f32 },

    /// Both pairs sharing the first arm conducted, which means the pair of the other two arms
    /// has to as well - yet it did not.
    ConductionInconsistency,

    /// An arm was driven through an unusable series resistance, which makes the loop
    /// current - and with it every resistance derived from it - impossible to determine.
    InvalidPullResistance { pull_resistance: f32 },
}

/// What the solve needs next.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SolveStep {
    /// Drive `high` to the high rail and `low` to the low rail, leave the remaining arm
    /// floating, then hand the resulting [`PairMeasurement`] back through
    /// [`SolveSequence::record`].
    MeasurePair { high: usize, low: usize },

    /// Every measurement the network needed has been taken.
    Finished(Result<ArmResistances, SolveError>),
}

/// Drives the measurement sequence of one star network: it hands out the pair to measure
/// next, takes that pair's result back, and resolves the arm resistances as soon as enough
/// of the network is known.
///
/// Keeping the sequence separate from the hardware lets the identical state machine run
/// against a simulated network in tests and against the real buffers and ADC chain on the
/// device.
pub struct SolveSequence {
    config: SolverConfig,
    measurements: [Option<PairMeasurement>; ARM_COUNT],
}

impl SolveSequence {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            measurements: [None; ARM_COUNT],
        }
    }

    /// The next step, which stays the same until its measurement is recorded.
    pub fn step(&self) -> SolveStep {
        let (Some(first), Some(second)) = (self.measurements[0], self.measurements[1]) else {
            return self.measure_pair();
        };

        let resistances = match (first.conducts, second.conducts) {
            (true, true) => return self.resolve_from_both(first, second),

            // Arm 2 is isolated, so only the first pair has anything to say.
            (true, false) => self.resolve_from_single(
                first,
                [0.0, 0.0, f32::INFINITY],
                [f32::NAN, f32::NAN, f32::INFINITY],
            ),

            // Arm 1 is isolated, so only the second pair has anything to say.
            (false, true) => self.resolve_from_single(
                second,
                [0.0, f32::INFINITY, 0.0],
                [f32::NAN, f32::INFINITY, f32::NAN],
            ),

            // Arm 0 was driven high in both pairs, so its being isolated would explain
            // neither of them conducting just as well as nothing being connected at all.
            // Only a pair that does not involve it can tell those two apart.
            (false, false) => match self.measurements[2] {
                None => return self.measure_pair(),
                Some(third) if third.conducts => self.resolve_from_single(
                    third,
                    [f32::INFINITY, 0.0, 0.0],
                    [f32::INFINITY, f32::NAN, f32::NAN],
                ),
                Some(_) => ArmResistances::DISCONNECTED,
            },
        };

        SolveStep::Finished(Ok(resistances))
    }

    /// Records the measurement of the pair [`step`](Self::step) last handed out.
    pub fn record(&mut self, measurement: PairMeasurement) {
        if let Some(slot) = self.measurements.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(measurement);
        }
    }

    fn measure_pair(&self) -> SolveStep {
        let (high, low) = PAIR_SEQUENCE[self.taken()];

        SolveStep::MeasurePair { high, low }
    }

    fn taken(&self) -> usize {
        self.measurements
            .iter()
            .take_while(|measurement| measurement.is_some())
            .count()
    }

    /// Resolves a network in which both pairs sharing the first arm conduct, measuring the
    /// third pair first if their voltage ratios are not enough on their own.
    fn resolve_from_both(&self, first: PairMeasurement, second: PairMeasurement) -> SolveStep {
        if let Some(resistances) = resolve::from_shorted_pairs(first, second, &self.config) {
            return SolveStep::Finished(Ok(resistances));
        }

        let first = Self::driven(0, first);
        let second = Self::driven(1, second);
        let ratios = resolve::ratios(first, second);
        if resolve::conditioning(&ratios) >= self.config.min_ratio_conditioning {
            return SolveStep::Finished(resolve::from_ratios(
                ratios,
                &[first, second],
                &self.config,
            ));
        }

        // The first arm is too close to 0 kΩ for its two pairs to compare the other two arms,
        // so only the pair made of exactly those two can.
        let Some(third) = self.measurements[2] else {
            return self.measure_pair();
        };
        if !third.conducts {
            return SolveStep::Finished(Err(SolveError::ConductionInconsistency));
        }

        let third = Self::driven(2, third);
        let with_first = resolve::ratios(first, third);
        let with_second = resolve::ratios(second, third);
        let ratios = if resolve::conditioning(&with_first) >= resolve::conditioning(&with_second) {
            with_first
        } else {
            with_second
        };

        SolveStep::Finished(resolve::from_ratios(
            ratios,
            &[first, second, third],
            &self.config,
        ))
    }

    fn resolve_from_single(
        &self,
        measurement: PairMeasurement,
        shorted: [f32; ARM_COUNT],
        unresolved: [f32; ARM_COUNT],
    ) -> ArmResistances {
        resolve::from_single_pair(measurement, shorted, unresolved, &self.config)
    }

    fn driven(index: usize, measurement: PairMeasurement) -> DrivenPair {
        let (high, low) = PAIR_SEQUENCE[index];

        DrivenPair {
            high,
            low,
            measurement,
        }
    }
}
