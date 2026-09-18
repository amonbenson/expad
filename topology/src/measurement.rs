use crate::config::SolverConfig;
use crate::sequence::SolveError;

/// Tap voltages read while one pair of arms was driven, in V.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PairVoltages {
    /// Tap of the arm driven to the high rail.
    pub high: f32,
    /// Tap of the arm driven to the low rail.
    pub low: f32,
    /// Tap of the arm left floating, which carries no current and therefore sits at the
    /// star point voltage.
    pub floating: f32,
}

/// What one arm's driver contributes while that arm is pulled to a rail.
///
/// Both are apparatus constants rather than properties of the network being measured:
/// `rail_voltage` is what this arm's tap reads while it is driven with the other two arms
/// floating, so no current flows and the tap sits exactly at its own rail.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ArmDrive {
    /// Rail voltage measured at this arm's own tap, in V.
    pub rail_voltage: f32,
    /// Series resistance between the driver and this arm's tap, in kΩ.
    pub pull_resistance: f32,
}

/// One pair of arms driven against each other, reduced to the current through them, their
/// combined resistance and how that resistance is split between them.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PairMeasurement {
    pub conducts: bool,

    /// Loop current through both driven arms, in mA.
    pub current: f32,

    /// Variance of `current` that the voltage noise alone accounts for, in mA².
    pub current_variance: f32,

    /// Combined resistance of both driven arms, in kΩ. Only as precise as `current`.
    pub pair_resistance: f32,

    /// Fraction of `pair_resistance` sitting in the arm driven to the low rail. It comes from
    /// tap voltages alone, which span most of the rail, so it stays precise even when the
    /// current is too small to measure well.
    pub low_resistance_fraction: f32,
}

impl PairMeasurement {
    /// No current flows between the two driven arms, so at least one of them is isolated.
    pub const OPEN: Self = Self {
        conducts: false,
        current: 0.0,
        current_variance: 0.0,
        pair_resistance: f32::INFINITY,
        low_resistance_fraction: f32::NAN,
    };

    /// Reduces the three tap voltages of one pair measurement.
    ///
    /// The current through the high side's pull resistor and the current through the low
    /// side's are the same current - two points on one loop - so comparing them catches an
    /// inconsistent reading before it can corrupt the result.
    pub fn from_voltages(
        voltages: PairVoltages,
        high: ArmDrive,
        low: ArmDrive,
        config: &SolverConfig,
    ) -> Result<Self, SolveError> {
        let high_current = high.current(high.rail_voltage - voltages.high)?;
        let low_current = low.current(voltages.low - low.rail_voltage)?;

        // Each side's current is one tap reading's voltage drop over its pull resistor, so
        // that is also all the noise it carries.
        let high_variance = high.current_variance(config);
        let low_variance = low.current_variance(config);

        let difference = high_current - low_current;
        let current = (high_current + low_current) / 2.0;
        let within_noise = difference * difference
            <= squared(config.current_consistency_sigmas) * (high_variance + low_variance);
        let within_tolerance =
            difference.abs() <= config.current_consistency_fraction * current.abs();
        if !within_noise && !within_tolerance {
            return Err(SolveError::CurrentConsistency {
                high_current,
                low_current,
            });
        }

        let current_variance = (high_variance + low_variance) / 4.0;
        let conducts = current > 0.0
            && squared(current) > squared(config.open_current_sigmas) * current_variance;
        if !conducts {
            return Ok(Self::OPEN);
        }

        // The floating tap sits at the star point, so how far it has risen above the low
        // tap is exactly the low arm's share of the span across both driven arms.
        let voltage_span = voltages.high - voltages.low;
        let low_resistance_fraction = if voltage_span > config.ratio_division_epsilon {
            (voltages.floating - voltages.low) / voltage_span
        } else {
            f32::NAN
        };

        Ok(Self {
            conducts: true,
            current,
            current_variance,
            pair_resistance: voltage_span / current,
            low_resistance_fraction,
        })
    }
}

impl ArmDrive {
    /// Current that `pull_drop`, the voltage dropped across this arm's pull resistor,
    /// implies is flowing through it, in mA.
    fn current(&self, pull_drop: f32) -> Result<f32, SolveError> {
        let known_pull_resistance = self.pull_resistance.is_finite() && self.pull_resistance > 0.0;
        if !known_pull_resistance {
            return Err(SolveError::InvalidPullResistance {
                pull_resistance: self.pull_resistance,
            });
        }

        Ok(pull_drop / self.pull_resistance)
    }

    /// Variance a single tap reading's noise puts on [`current`](Self::current), in mA².
    fn current_variance(&self, config: &SolverConfig) -> f32 {
        squared(config.voltage_noise / self.pull_resistance)
    }
}

pub(crate) fn squared(value: f32) -> f32 {
    value * value
}
