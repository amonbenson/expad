//! Drives the solver from simulated networks with known arm resistances. Ported from the
//! simulator's `src/core/network/solver.test.ts`.

mod star_network;

use expad_topology::{
    ARM_COUNT, ArmResistances, MonitorConfig, Network, PairMeasurement, PairVoltages, SolveError,
    SolveSequence, SolveStep, SolverConfig,
};
use star_network::{Role, StarNetwork};

/// Relative tolerance for solved resistances. The solver works in f32, and deriving a loop
/// current from the small difference between two tap voltages costs significant digits, so
/// high-resistance networks land noticeably wider than f32's own precision.
const TOLERANCE: f32 = 1e-3;
const HIGH_RESISTANCE_TOLERANCE: f32 = 1e-2;

/// Voltage offset injected into a tap reading to simulate an inconsistent one. Well above
/// the default noise floor the solver's tolerances are derived from.
const INCONSISTENT_OFFSET: f32 = 0.05;

/// Voltage offset of one standard deviation of the default noise floor.
const NOISE_OFFSET: f32 = SolverConfig::DEFAULT_VOLTAGE_NOISE;

/// How a jack monitor with the default configuration classifies `resistances`.
fn network(resistances: &ArmResistances) -> Network {
    let config = MonitorConfig::default();
    resistances.network(config.max_wiper_relative, config.end_stop_relative)
}

/// Wiper position of `resistances` if they form a potentiometer with a clear wiper.
fn wiper_position(resistances: &ArmResistances) -> Option<f32> {
    match network(resistances) {
        Network::Potentiometer { wiper } => resistances.position_with_wiper(wiper),
        _ => None,
    }
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= tolerance * expected.abs().max(1.0),
        "expected {expected}, got {actual}"
    );
}

fn solve(network: &StarNetwork) -> Result<ArmResistances, SolveError> {
    solve_with_offsets(network, [0.0; ARM_COUNT])
}

/// Runs the whole measurement sequence against `network`, adding `offsets` to every reading
/// of the matching arm's tap to stand in for what an imperfect ADC reads.
fn solve_with_offsets(
    network: &StarNetwork,
    offsets: [f32; ARM_COUNT],
) -> Result<ArmResistances, SolveError> {
    let config = SolverConfig::default();
    let mut sequence = SolveSequence::new(config);

    loop {
        match sequence.step() {
            SolveStep::MeasurePair { high, low } => {
                let floating = ARM_COUNT - high - low;

                let mut roles = [Role::Floating; ARM_COUNT];
                roles[high] = Role::High;
                roles[low] = Role::Low;

                let mut taps = network.tap_voltages(roles);
                for (tap, offset) in taps.iter_mut().zip(offsets) {
                    *tap += offset;
                }

                sequence.record(PairMeasurement::from_voltages(
                    PairVoltages {
                        high: taps[high],
                        low: taps[low],
                        floating: taps[floating],
                    },
                    network.drive(high, Role::High),
                    network.drive(low, Role::Low),
                    &config,
                )?);
            }
            SolveStep::Finished(result) => return result,
        }
    }
}

#[test]
fn resolves_a_generic_three_finite_arm_topology_into_ratios_and_a_total() {
    let arms = [10.0, 22.0, 47.0];
    let total = arms[0] + arms[1] + arms[2];

    let resistances = solve(&StarNetwork::new(arms)).unwrap();

    assert_close(resistances.relative[0], arms[0] / total, TOLERANCE);
    assert_close(resistances.relative[1], arms[1] / total, TOLERANCE);
    assert_close(resistances.relative[2], arms[2] / total, TOLERANCE);
    assert_close(resistances.total, total, TOLERANCE);
}

#[test]
fn recovers_each_arms_actual_resistance_from_the_total_times_its_fraction() {
    let arms = [150.0, 330.0, 220.0];

    let resistances = solve(&StarNetwork::new(arms)).unwrap();

    for (arm, &resistance) in arms.iter().enumerate() {
        assert_close(resistances.absolute(arm), resistance, TOLERANCE);
    }
}

/// Every measured fraction belongs to one specific arm of its pair, and with arms this
/// asymmetric, attributing any of them to the wrong arm cannot hide: it lands the
/// resistances on the wrong arms.
#[test]
fn assigns_every_resistance_to_the_arm_it_belongs_to() {
    let arms = [10.0, 220.0, 47.0];
    let total = arms[0] + arms[1] + arms[2];

    let resistances = solve(&StarNetwork::new(arms)).unwrap();

    assert_close(resistances.absolute(0), 10.0, TOLERANCE);
    assert_close(resistances.absolute(1), 220.0, TOLERANCE);
    assert_close(resistances.absolute(2), 47.0, TOLERANCE);
    assert_close(resistances.total, total, TOLERANCE);
}

#[test]
fn measures_the_actual_high_rail_voltage_instead_of_assuming_one() {
    let network = StarNetwork::new([10.0, 22.0, 47.0]).with_rails(0.0, 9.0);

    assert_close(network.drive(0, Role::High).rail_voltage, 9.0, TOLERANCE);
    assert_close(solve(&network).unwrap().total, 79.0, TOLERANCE);
}

#[test]
fn measures_the_actual_low_rail_voltage_instead_of_assuming_it_is_zero() {
    let network = StarNetwork::new([10.0, 22.0, 47.0]).with_rails(1.2, 4.5);

    assert_close(network.drive(0, Role::Low).rail_voltage, 1.2, TOLERANCE);

    let resistances = solve(&network).unwrap();

    assert_close(resistances.relative[0], 10.0 / 79.0, TOLERANCE);
    assert_close(resistances.relative[1], 22.0 / 79.0, TOLERANCE);
    assert_close(resistances.relative[2], 47.0 / 79.0, TOLERANCE);
    assert_close(resistances.total, 79.0, TOLERANCE);
}

#[test]
fn uses_whatever_pull_resistance_the_arms_are_actually_driven_through() {
    let network = StarNetwork::new([10.0, 22.0, 47.0]).with_pull_resistances([5.0; ARM_COUNT]);

    let resistances = solve(&network).unwrap();

    assert_close(resistances.relative[0], 10.0 / 79.0, TOLERANCE);
    assert_close(resistances.total, 79.0, TOLERANCE);
}

/// Exercises the generic resolution path's consistency check at exactly 0/0: arm 0 is a
/// genuine short, but both its pairs (0+100 and 0+220) stay well above the short
/// threshold, so neither shortcut catches it first.
#[test]
fn resolves_a_shared_arm_of_exactly_zero_as_a_regular_case_not_a_short() {
    let resistances = solve(&StarNetwork::new([0.0, 100.0, 220.0])).unwrap();

    assert_close(resistances.relative[0], 0.0, TOLERANCE);
    assert_close(resistances.relative[1], 100.0 / 320.0, TOLERANCE);
    assert_close(resistances.relative[2], 220.0 / 320.0, TOLERANCE);
    assert_close(resistances.total, 320.0, TOLERANCE);
}

#[test]
fn resolves_from_the_first_pair_alone_when_arm_two_is_isolated() {
    let resistances = solve(&StarNetwork::new([100.0, 900.0, f32::INFINITY])).unwrap();

    assert!(resistances.relative[0].is_nan());
    assert!(resistances.relative[1].is_nan());
    assert_eq!(resistances.relative[2], f32::INFINITY);
    assert_close(resistances.total, 1000.0, HIGH_RESISTANCE_TOLERANCE);
}

#[test]
fn resolves_from_the_second_pair_alone_when_arm_one_is_isolated() {
    let resistances = solve(&StarNetwork::new([47.0, f32::INFINITY, 100.0])).unwrap();

    assert!(resistances.relative[0].is_nan());
    assert_eq!(resistances.relative[1], f32::INFINITY);
    assert!(resistances.relative[2].is_nan());
    assert_close(resistances.total, 147.0, TOLERANCE);
}

/// Arm 0 is driven high in both of the first two pairs, so neither conducts when it is
/// isolated - exactly as when nothing is connected at all. Only the third pair, which
/// leaves arm 0 out, tells the two apart.
#[test]
fn falls_back_to_a_third_pair_when_arm_zero_is_isolated() {
    let resistances = solve(&StarNetwork::new([f32::INFINITY, 33.0, 47.0])).unwrap();

    assert_eq!(resistances.relative[0], f32::INFINITY);
    assert!(resistances.relative[1].is_nan());
    assert!(resistances.relative[2].is_nan());
    assert_close(resistances.total, 80.0, TOLERANCE);
}

#[test]
fn reports_every_arm_as_open_when_nothing_is_connected() {
    let resistances = solve(&StarNetwork::new([f32::INFINITY; ARM_COUNT])).unwrap();

    assert_eq!(resistances.relative, [f32::INFINITY; ARM_COUNT]);
    assert!(resistances.total.is_nan());
}

#[test]
fn detects_a_short_between_arms_zero_and_one_with_arm_two_still_attached() {
    let resistances = solve(&StarNetwork::new([0.0, 0.0, 50.0])).unwrap();

    assert_eq!(resistances.relative, [0.0, 0.0, 1.0]);
    assert_close(resistances.total, 50.0, TOLERANCE);
}

#[test]
fn detects_a_short_between_arms_zero_and_two_from_the_pair_that_drives_them() {
    let resistances = solve(&StarNetwork::new([0.0, 470.0, 0.0])).unwrap();

    assert_eq!(resistances.relative, [0.0, 1.0, 0.0]);
    assert_close(resistances.total, 470.0, TOLERANCE);
}

/// No measured pair drives arms 1 and 2 against each other, so this short has to fall out
/// of the generic resolution instead of being spotted directly.
#[test]
fn detects_a_short_between_arms_one_and_two_through_the_generic_path() {
    let resistances = solve(&StarNetwork::new([470.0, 0.0, 0.0])).unwrap();

    assert_close(resistances.relative[0], 1.0, TOLERANCE);
    assert_close(resistances.relative[1], 0.0, TOLERANCE);
    assert_close(resistances.relative[2], 0.0, TOLERANCE);
    assert_close(resistances.total, 470.0, TOLERANCE);
}

#[test]
fn detects_a_short_between_arms_zero_and_one_with_arm_two_isolated() {
    let resistances = solve(&StarNetwork::new([0.0, 0.0, f32::INFINITY])).unwrap();

    assert_eq!(resistances.relative, [0.0, 0.0, f32::INFINITY]);
    assert!(resistances.total.is_nan());
}

#[test]
fn detects_all_three_arms_shorted_together() {
    let resistances = solve(&StarNetwork::new([0.0; ARM_COUNT])).unwrap();

    assert_eq!(resistances.relative, [0.0; ARM_COUNT]);
    assert!(resistances.total.is_nan());
}

#[test]
fn rejects_a_measurement_whose_loop_current_disagrees_between_its_two_sides() {
    let network = StarNetwork::new([100.0, 200.0, 300.0]);

    let error = solve_with_offsets(&network, [INCONSISTENT_OFFSET, 0.0, 0.0]).unwrap_err();

    assert!(matches!(error, SolveError::CurrentConsistency { .. }));
}

/// Two measurements that are individually well-formed and agree on how the arms compare
/// (1 : 2 : 2), but whose precisely measured currents imply totals of 500 kΩ and 750 kΩ -
/// which no single network can produce, but a pedal moved between them can.
#[test]
fn rejects_two_measurements_that_disagree_about_the_total_resistance() {
    let mut sequence = SolveSequence::new(SolverConfig::default());

    sequence.record(PairMeasurement {
        conducts: true,
        current: 0.01,
        current_variance: 1e-12,
        pair_resistance: 300.0,
        low_resistance_fraction: 200.0 / 300.0,
    });
    sequence.record(PairMeasurement {
        conducts: true,
        current: 0.01,
        current_variance: 1e-12,
        pair_resistance: 450.0,
        low_resistance_fraction: 300.0 / 450.0,
    });

    let SolveStep::Finished(result) = sequence.step() else {
        panic!("expected the sequence to be finished after two conducting pairs");
    };

    assert!(matches!(
        result.unwrap_err(),
        SolveError::ResistanceConsistency { .. }
    ));
}

/// The wiper sits on the shared arm here, so both pairs sharing it only report "all of it
/// is in the other arm" - the third pair, across the two track ends, has to split them. The
/// position counts from the sleeve (arm 2) end.
#[test]
fn reads_a_potentiometers_wiper_position_from_its_two_track_halves() {
    let resistances = solve(&StarNetwork::new([0.0, 3.0, 7.0])).unwrap();

    let position = wiper_position(&resistances)
        .expect("a shorted arm between two track halves is a potentiometer");

    assert_close(position, 0.7, TOLERANCE);
}

/// The breadboard's potentiometer: about 1 MΩ with the wiper a third of the way along, so the
/// whole track carries barely 3 µA - a drop of a few millivolts over a 1 kΩ pull resistor.
#[test]
fn resolves_a_high_value_potentiometer_whose_currents_are_barely_above_the_noise() {
    let resistances = solve(&StarNetwork::new([300.0, 0.0, 700.0])).unwrap();

    let position = wiper_position(&resistances)
        .expect("a shorted arm between two track halves is a potentiometer");

    assert_close(position, 0.7, TOLERANCE);
    assert_close(resistances.total, 1000.0, HIGH_RESISTANCE_TOLERANCE);
}

/// Readings off by one standard deviation of noise skew the few-microampere currents of a
/// high-value potentiometer by a third, but barely move the voltage ratios its position
/// comes from.
#[test]
fn keeps_the_position_accurate_when_the_currents_are_dominated_by_noise() {
    let network = StarNetwork::new([300.0, 0.0, 700.0]);

    let resistances =
        solve_with_offsets(&network, [NOISE_OFFSET, -NOISE_OFFSET, NOISE_OFFSET]).unwrap();

    let position = wiper_position(&resistances)
        .expect("a shorted arm between two track halves is a potentiometer");

    assert_close(position, 0.7, HIGH_RESISTANCE_TOLERANCE);
}

/// The PCB's 10.85 kΩ pedal a fifth of the way along, its wiper on the shared arm with 1.5%
/// contact resistance. The two pairs sharing the wiper are just well-conditioned enough to
/// look usable, but split the track halves through a drop of a few millivolts across the
/// wiper - one standard deviation of noise moved the position by most of a MIDI step, until
/// the solver was made to take the third pair across the two track ends instead.
#[test]
fn keeps_the_position_steady_when_the_wiper_contact_resistance_is_the_shared_arm() {
    let network = StarNetwork::new([0.16, 2.3, 8.4]);
    let expected_position = 8.4 / (2.3 + 8.4);
    let quarter_midi_step = 0.25 / 127.0;

    for offsets in [
        [NOISE_OFFSET, -NOISE_OFFSET, NOISE_OFFSET],
        [-NOISE_OFFSET, NOISE_OFFSET, -NOISE_OFFSET],
        [NOISE_OFFSET, NOISE_OFFSET, -NOISE_OFFSET],
        [-NOISE_OFFSET, -NOISE_OFFSET, NOISE_OFFSET],
    ] {
        let resistances = solve_with_offsets(&network, offsets).unwrap();

        let position = wiper_position(&resistances)
            .expect("a near-shorted arm between two track halves is a potentiometer");

        assert_close(position, expected_position, quarter_midi_step);
    }
}

#[test]
fn classifies_a_network_with_no_arm_at_the_star_point_as_other() {
    let resistances = solve(&StarNetwork::new([10.0, 22.0, 47.0])).unwrap();

    assert_eq!(network(&resistances), Network::Other);
    assert_eq!(wiper_position(&resistances), None);
    assert_eq!(
        network(&ArmResistances::DISCONNECTED),
        Network::Disconnected
    );
}

/// A wiper resting on the sleeve end shorts the sleeve to the star point as well, so either
/// could be the wiper.
#[test]
fn classifies_a_wiper_resting_on_a_track_end_as_an_end_stop() {
    let resistances = solve(&StarNetwork::new([10.0, 0.0, 0.0])).unwrap();

    assert!(matches!(
        network(&resistances),
        Network::EndStop { candidates } if candidates.contains(&1) && candidates.contains(&2)
    ));
}

/// A mono plug's sleeve shorts the ring to it, whatever sits between tip and sleeve; a stereo
/// plug leaves the ring isolated.
#[test]
fn classifies_a_single_element_between_tip_and_sleeve() {
    let closed_mono = solve(&StarNetwork::new([0.0, 0.0, 0.0])).unwrap();
    let open_mono = solve(&StarNetwork::new([f32::INFINITY, 0.0, 0.0])).unwrap();
    let stereo = solve(&StarNetwork::new([5.0, f32::INFINITY, 0.0])).unwrap();

    for mono in [closed_mono, open_mono] {
        assert_eq!(network(&mono), Network::TipSleeve { ring_shorted: true });
    }
    assert_eq!(
        network(&stereo),
        Network::TipSleeve {
            ring_shorted: false
        }
    );
}
