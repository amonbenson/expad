use crate::hal::adc::{AdcChain, AdcChainError};
use crate::hal::buf::{QuadBufferChain, QuadBufferChainError, TriState};

#[derive(Debug, Clone, Copy)]
pub enum ResistanceSolverError {
    QuadBufferChain(QuadBufferChainError),
    AdcChain(AdcChainError),
    CurrentConsistency {
        high_current: f32,
        low_current: f32,
    },
    ResistanceConsistency {
        shared_arm_disagreement: f32,
        share_arm_resistance_sum: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ArmResistances {
    /// Relative resistances of each arm, normalized such that the sum of all three is 1.0.
    pub relative: [f32; 3],

    /// Total absolute resistance of all three arms in series (in kΩ).
    pub total: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PairMeasurement {
    pub conducts: bool,
    pub pair_resistance: f32,
    pub low_resistance_fraction: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ArmConfig {
    pub quadbuf_chip: usize,
    pub quadbuf_channel: u8,
    pub adc_chip: usize,
    pub adc_channel: u8,
    pub pull_up_resistance: f32,
    pub pull_down_resistance: f32,
}

impl Default for ArmConfig {
    fn default() -> Self {
        ArmConfig {
            quadbuf_chip: 0,
            quadbuf_channel: 0,
            adc_chip: 0,
            adc_channel: 0,
            pull_up_resistance: 1.0,
            pull_down_resistance: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResistanceSolverConfig {
    pub open_current_threshold: f32,
    pub short_resistance_threshold: f32,
    pub current_consistency_threshold: f32,
    pub resistance_consistency_threshold: f32,
    pub resistance_sum_epsilon: f32,
    pub ratio_division_epsilon: f32,
}

impl Default for ResistanceSolverConfig {
    fn default() -> Self {
        ResistanceSolverConfig {
            open_current_threshold: 1e-9,
            short_resistance_threshold: 1e-6,
            current_consistency_threshold: 1e-9,
            resistance_consistency_threshold: 1e-6,
            resistance_sum_epsilon: 1e-9,
            ratio_division_epsilon: 1e-12,
        }
    }
}

pub struct ResistanceSolver<'d, const N_QUADBUFS: usize, const N_ADCS: usize> {
    config: ResistanceSolverConfig,
    quadbufs: QuadBufferChain<'d, N_QUADBUFS>,
    adcs: AdcChain<'d, N_ADCS>,

    high_rail_voltage: f32,
    low_rail_voltage: f32,
}

impl<'d, const N_QUADBUFS: usize, const N_ADCS: usize> ResistanceSolver<'d, N_QUADBUFS, N_ADCS> {
    pub fn new(
        config: ResistanceSolverConfig,
        quadbufs: QuadBufferChain<'d, N_QUADBUFS>,
        adcs: AdcChain<'d, N_ADCS>,
    ) -> Self {
        ResistanceSolver {
            config,
            quadbufs,
            adcs,

            // Todo: read from ADC calibration register OR measure directly (?)
            high_rail_voltage: 3.3,
            low_rail_voltage: 0.0,
        }
    }

    pub async fn solve(
        &mut self,
        arms: [ArmConfig; 3],
    ) -> Result<ArmResistances, ResistanceSolverError> {
        let first_measurement = self.measure_pair(arms, 0, 1).await?;
        let second_measurement = self.measure_pair(arms, 0, 2).await?;

        if first_measurement.conducts && second_measurement.conducts {
            self.resolve_from_both_pairs(first_measurement, second_measurement)
        } else if first_measurement.conducts {
            // Arm 2 is infinite (isolated)
            self.resolve_from_single_pair(
                first_measurement,
                [0.0, 0.0, f32::INFINITY],
                [f32::NAN, f32::NAN, f32::INFINITY],
            )
        } else if second_measurement.conducts {
            // Arm 1 is infinite (isolated)
            self.resolve_from_single_pair(
                second_measurement,
                [0.0, f32::INFINITY, 0.0],
                [f32::NAN, f32::INFINITY, f32::NAN],
            )
        } else {
            // Neither measurement conducted, so arm 0 must be isolated. Confirm with a third measurement.
            let third_measurement = self.measure_pair(arms, 1, 2).await?;
            if third_measurement.conducts {
                // Arm 0 is infinite (isolated)
                self.resolve_from_single_pair(
                    third_measurement,
                    [f32::INFINITY, 0.0, 0.0],
                    [f32::INFINITY, f32::NAN, f32::NAN],
                )
            } else {
                // All three arms are isolated from each other ("nothing connected")
                Ok(ArmResistances {
                    relative: [f32::INFINITY, f32::INFINITY, f32::INFINITY],
                    total: f32::NAN,
                })
            }
        }
    }

    fn resolve_from_both_pairs(
        &self,
        first_measurement: PairMeasurement,
        second_measurement: PairMeasurement,
    ) -> Result<ArmResistances, ResistanceSolverError> {
        let first_pair_shorted =
            first_measurement.pair_resistance < self.config.short_resistance_threshold;
        let second_pair_shorted =
            second_measurement.pair_resistance < self.config.short_resistance_threshold;

        if first_pair_shorted && second_pair_shorted {
            return Ok(ArmResistances {
                relative: [0.0; 3],
                total: 0.0,
            });
        }

        if first_pair_shorted {
            // Arms 0 and 1 are shorted, so arm 2 holds all the resistance
            return Ok(ArmResistances {
                relative: [0.0, 0.0, 1.0],
                total: second_measurement.pair_resistance,
            });
        }

        if second_pair_shorted {
            // Arms 0 and 2 are shorted, so arm 1 holds all the resistance
            return Ok(ArmResistances {
                relative: [0.0, 1.0, 0.0],
                total: first_measurement.pair_resistance,
            });
        }

        let second_arm_resistance =
            first_measurement.low_resistance_fraction * first_measurement.pair_resistance;
        let third_arm_resistance =
            second_measurement.low_resistance_fraction * second_measurement.pair_resistance;
        let shared_arm_resistance_from_first_measurement =
            (1.0 - first_measurement.low_resistance_fraction) * first_measurement.pair_resistance;
        let shared_arm_resistance_from_second_measurement =
            (1.0 - second_measurement.low_resistance_fraction) * second_measurement.pair_resistance;

        // Both measurements drove arm 0, so they each produce an independent estimate of its resistance.
        // We can also use this as a self-consistency check
        let shared_arm_resistance_sum = shared_arm_resistance_from_first_measurement
            + shared_arm_resistance_from_second_measurement;
        let shared_arm_disagreement = (shared_arm_resistance_from_first_measurement
            - shared_arm_resistance_from_second_measurement)
            .abs();
        if shared_arm_disagreement
            / (shared_arm_resistance_sum + self.config.resistance_sum_epsilon)
            > self.config.resistance_consistency_threshold
        {
            return Err(ResistanceSolverError::ResistanceConsistency {
                shared_arm_disagreement,
                share_arm_resistance_sum: shared_arm_resistance_sum,
            });
        }

        let shared_arm_resistance = shared_arm_resistance_sum / 2.0;
        let total_resistance = shared_arm_resistance + second_arm_resistance + third_arm_resistance;

        Ok(ArmResistances {
            relative: [
                shared_arm_resistance / total_resistance,
                second_arm_resistance / total_resistance,
                third_arm_resistance / total_resistance,
            ],
            total: total_resistance,
        })
    }

    fn resolve_from_single_pair(
        &self,
        measurement: PairMeasurement,
        shorted_arms: [f32; 3],
        unresolved_arms: [f32; 3],
    ) -> Result<ArmResistances, ResistanceSolverError> {
        if measurement.pair_resistance < self.config.short_resistance_threshold {
            Ok(ArmResistances {
                relative: shorted_arms,
                total: f32::NAN,
            })
        } else {
            Ok(ArmResistances {
                relative: unresolved_arms,
                total: measurement.pair_resistance,
            })
        }
    }

    async fn measure_pair(
        &mut self,
        arms: [ArmConfig; 3],
        low_index: usize,
        high_index: usize,
    ) -> Result<PairMeasurement, ResistanceSolverError> {
        let floating_index = 3 - low_index - high_index;

        let high_arm = arms[high_index];
        let low_arm = arms[low_index];
        let floating_arm = arms[floating_index];

        // Set the arm states
        self.quadbufs.set_output(
            high_arm.quadbuf_chip,
            high_arm.quadbuf_channel,
            TriState::High,
        );
        self.quadbufs
            .set_output(low_arm.quadbuf_chip, low_arm.quadbuf_channel, TriState::Low);
        self.quadbufs.set_output(
            floating_arm.quadbuf_chip,
            floating_arm.quadbuf_channel,
            TriState::HiZ,
        );
        self.quadbufs
            .update()
            .map_err(ResistanceSolverError::QuadBufferChain)?;

        // Measure the voltage on the floating arm
        let high_voltage = self
            .adcs
            .measure_channel(high_arm.adc_chip, high_arm.adc_channel)
            .await
            .map_err(ResistanceSolverError::AdcChain)?;
        let low_voltage = self
            .adcs
            .measure_channel(low_arm.adc_chip, low_arm.adc_channel)
            .await
            .map_err(ResistanceSolverError::AdcChain)?;
        let floating_voltage = self
            .adcs
            .measure_channel(floating_arm.adc_chip, floating_arm.adc_channel)
            .await
            .map_err(ResistanceSolverError::AdcChain)?;

        // Assuming no current flows into the ADC inputs, the current through the high arm pull up resistor must equal the current through the low arm pull down resistor.
        // We can use this as a self-consistency check to determine if the measurement is valid.
        let high_current = (self.high_rail_voltage - high_voltage) / high_arm.pull_up_resistance;
        let low_current = (low_voltage - self.low_rail_voltage) / low_arm.pull_down_resistance;

        if (high_current - low_current).abs() > self.config.current_consistency_threshold {
            return Err(ResistanceSolverError::CurrentConsistency {
                high_current,
                low_current,
            });
        }

        // Detect open circuit on either arm
        let current = (high_current + low_current) / 2.0;
        if current <= self.config.open_current_threshold {
            return Ok(PairMeasurement {
                conducts: false,
                pair_resistance: f32::INFINITY,
                low_resistance_fraction: f32::NAN,
            });
        }

        // Calculate the pair resistance and the fraction of the total resistance in the low arm.
        let voltage_span = high_voltage - low_voltage;
        let pair_resistance = voltage_span / current;
        let low_resistance_fraction = if voltage_span.abs() > self.config.ratio_division_epsilon {
            (floating_voltage - low_voltage) / voltage_span
        } else {
            f32::NAN
        };

        Ok(PairMeasurement {
            conducts: true,
            pair_resistance,
            low_resistance_fraction,
        })
    }
}
