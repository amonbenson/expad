/// Tolerances the solver judges measurements by.
///
/// Everything that depends on how noisy the readings are is expressed in standard deviations
/// of that noise, which the solver derives per measurement from `voltage_noise` and the pull
/// resistances actually involved. A low-current measurement - a high-value potentiometer, say -
/// therefore gets proportionally wider tolerances instead of being rejected for noise the
/// hardware cannot avoid. Adapting to a different front end only takes
/// [`SolverConfig::from_voltage_noise`] with that front end's figure.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SolverConfig {
    /// Standard deviation of a single tap reading, in V. Rail voltages are assumed to be
    /// averaged over enough readings that their own noise is negligible next to it.
    pub voltage_noise: f32,

    /// Standard deviations of current noise a pair's loop current has to exceed to count as
    /// conducting. Together with the rail span and the pull resistances, this sets the
    /// largest pair resistance that can still be told apart from an open circuit.
    pub open_current_sigmas: f32,

    /// Standard deviations by which the high-side and low-side currents of one loop may
    /// disagree before the measurement is rejected as inconsistent.
    pub current_consistency_sigmas: f32,

    /// Fraction of the loop current by which the two sides may disagree regardless of noise,
    /// covering pull resistor tolerances.
    pub current_consistency_fraction: f32,

    /// Standard deviations by which the total resistance implied by one pair may disagree
    /// with the combined estimate before the pairs are rejected as describing different
    /// networks.
    pub total_consistency_sigmas: f32,

    /// Fraction of the total resistance the estimates may disagree by regardless of noise.
    pub total_consistency_fraction: f32,

    /// Largest pair resistance still treated as a dead short, in kΩ.
    pub short_resistance_threshold: f32,

    /// Smallest voltage span across a driven pair that still yields a meaningful position
    /// ratio, in V.
    pub ratio_division_epsilon: f32,

    /// How well two pairs' voltage ratios have to pin the arms down to be used on their own,
    /// as the sum of the unnormalized ratios they produce (`1.0` is perfect, `0.0` hopeless).
    /// It drops towards zero as the arm the two pairs share approaches 0 kΩ, which leaves
    /// them unable to say anything about the other two - a third pair is measured then.
    /// Noise in the ratios is amplified by roughly its inverse, so a potentiometer whose wiper
    /// is the shared arm (conditioning ~0.03-0.1, depending on the wiper's contact resistance)
    /// must always take the third pair: at 0.05 a 1.5% wiper passed, and its position jittered
    /// by 0.5% of travel instead of 0.02%.
    pub min_ratio_conditioning: f32,
}

impl SolverConfig {
    /// Standard deviation of a single AD7718 reading at an 819 Hz update rate, as measured on
    /// the expression controller PCB through a pedal.
    pub const DEFAULT_VOLTAGE_NOISE: f32 = 0.0005;

    /// Series resistance each arm is driven through on the expression controller, in kΩ.
    pub const DEFAULT_PULL_RESISTANCE: f32 = 1.0;

    /// Derives every tolerance from the noise of a single tap reading, a property of the
    /// measuring hardware rather than of the network being measured.
    pub fn from_voltage_noise(voltage_noise: f32) -> Self {
        Self {
            voltage_noise,
            open_current_sigmas: 3.0,
            current_consistency_sigmas: 5.0,
            current_consistency_fraction: 0.1,
            total_consistency_sigmas: 4.0,
            total_consistency_fraction: 0.1,
            short_resistance_threshold: 0.05,
            ratio_division_epsilon: 10.0 * voltage_noise,
            min_ratio_conditioning: 0.5,
        }
    }
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self::from_voltage_noise(Self::DEFAULT_VOLTAGE_NOISE)
    }
}
