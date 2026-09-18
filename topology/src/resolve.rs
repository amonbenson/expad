use crate::config::SolverConfig;
use crate::measurement::{PairMeasurement, squared};
use crate::resistances::{ARM_COUNT, ArmResistances};
use crate::sequence::SolveError;

/// A conducting pair measurement together with the arms it drove.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DrivenPair {
    pub high: usize,
    pub low: usize,
    pub measurement: PairMeasurement,
}

/// Resolves the cases where one of the two pairs sharing the first arm is a dead short,
/// which leaves no voltage ratio to split it by. `None` if neither is.
pub(crate) fn from_shorted_pairs(
    first: PairMeasurement,
    second: PairMeasurement,
    config: &SolverConfig,
) -> Option<ArmResistances> {
    let first_pair_shorted = first.pair_resistance < config.short_resistance_threshold;
    let second_pair_shorted = second.pair_resistance < config.short_resistance_threshold;

    match (first_pair_shorted, second_pair_shorted) {
        // Every arm is shorted to the star point, so there is no resistance left to
        // distribute and no total to report.
        (true, true) => Some(ArmResistances {
            relative: [0.0; ARM_COUNT],
            total: f32::NAN,
        }),

        // Arms 0 and 1 are shorted, so arm 2 holds the entire remaining resistance.
        (true, false) => Some(ArmResistances {
            relative: [0.0, 0.0, 1.0],
            total: second.pair_resistance,
        }),

        // Arms 0 and 2 are shorted, so arm 1 holds the entire remaining resistance.
        (false, true) => Some(ArmResistances {
            relative: [0.0, 1.0, 0.0],
            total: first.pair_resistance,
        }),

        (false, false) => None,
    }
}

/// Arm resistances, up to a common scale, that two pairs sharing one arm pin down through
/// their voltage ratios alone.
///
/// Each pair's floating tap fixes the ratio between its two arms without involving the
/// loop current, and two such ratios through a shared arm fix all three arms relative to
/// each other. How far the sum of the result falls short of `1.0` is how badly conditioned
/// that is: it drops to zero as the shared arm approaches 0 kΩ, when both ratios only say
/// "all of it is in the other arm".
pub(crate) fn ratios(first: DrivenPair, second: DrivenPair) -> [f32; ARM_COUNT] {
    let shared = if first.high == second.high || first.high == second.low {
        first.high
    } else {
        first.low
    };

    let (shared_in_first, first_other, first_other_share) = split_around(first, shared);
    let (shared_in_second, second_other, second_other_share) = split_around(second, shared);

    let mut ratios = [0.0; ARM_COUNT];
    ratios[shared] = shared_in_first * shared_in_second;
    ratios[first_other] = first_other_share * shared_in_second;
    ratios[second_other] = second_other_share * shared_in_first;

    ratios
}

/// How well `ratios` pin the arms down, from `0.0` (not at all) to `1.0`.
pub(crate) fn conditioning(ratios: &[f32; ARM_COUNT]) -> f32 {
    ratios.iter().sum()
}

/// Normalizes `ratios` into relative resistances and scales them by the total resistance
/// the pairs' currents imply, each trusted in proportion to its signal-to-noise ratio.
pub(crate) fn from_ratios(
    ratios: [f32; ARM_COUNT],
    pairs: &[DrivenPair],
    config: &SolverConfig,
) -> Result<ArmResistances, SolveError> {
    let conditioning = conditioning(&ratios);
    let relative = ratios.map(|ratio| ratio / conditioning);

    let (weighted_sum, weight_sum) = pairs
        .iter()
        .map(|pair| total_estimate(pair, &relative))
        .fold((0.0, 0.0), |(weighted_sum, weight_sum), (total, weight)| {
            (weighted_sum + total * weight, weight_sum + weight)
        });
    let total = weighted_sum / weight_sum;

    // Every pair measured the same network, so the totals they imply must agree within the
    // noise of their currents - a pedal moved or unplugged between two pairs does not.
    for pair in pairs {
        let (estimate, _) = total_estimate(pair, &relative);
        let deviation = estimate - total;
        let measurement = pair.measurement;

        let within_noise = squared(deviation)
            <= squared(config.total_consistency_sigmas * total)
                * (measurement.current_variance / squared(measurement.current));
        let within_tolerance = deviation.abs() <= config.total_consistency_fraction * total;
        if !within_noise && !within_tolerance {
            return Err(SolveError::ResistanceConsistency {
                total,
                disagreeing_total: estimate,
            });
        }
    }

    Ok(ArmResistances { relative, total })
}

/// Resolves what a single conducting pair can tell us while the third arm is isolated:
/// either both driven arms are shorted to the star point, or they are individually
/// unresolvable and only their combined resistance is known.
pub(crate) fn from_single_pair(
    measurement: PairMeasurement,
    shorted: [f32; ARM_COUNT],
    unresolved: [f32; ARM_COUNT],
    config: &SolverConfig,
) -> ArmResistances {
    if measurement.pair_resistance < config.short_resistance_threshold {
        ArmResistances {
            relative: shorted,
            total: f32::NAN,
        }
    } else {
        ArmResistances {
            relative: unresolved,
            total: measurement.pair_resistance,
        }
    }
}

/// `pair`'s split around `shared`, as that arm's share, the pair's other arm, and its share.
fn split_around(pair: DrivenPair, shared: usize) -> (f32, usize, f32) {
    let low_share = pair.measurement.low_resistance_fraction;

    if pair.high == shared {
        (1.0 - low_share, pair.low, low_share)
    } else {
        (low_share, pair.high, 1.0 - low_share)
    }
}

/// The total resistance `pair`'s current implies given the relative resistances, and how
/// much to trust it: its squared signal-to-noise ratio, i.e. its inverse relative variance.
fn total_estimate(pair: &DrivenPair, relative: &[f32; ARM_COUNT]) -> (f32, f32) {
    let measurement = pair.measurement;
    let share = relative[pair.high] + relative[pair.low];
    let weight = squared(measurement.current) / measurement.current_variance.max(f32::MIN_POSITIVE);

    (measurement.pair_resistance / share, weight)
}
