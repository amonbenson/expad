use embassy_time::{Duration, Timer};
use expad_topology::{
    ARM_COUNT, ArmDrive, ArmResistances, PairMeasurement, PairVoltages, SolveError, SolveSequence,
    SolveStep, SolverConfig,
};

use crate::hal::adc::{AdcChain, AdcChainError};
use crate::hal::buf::{PullSwitchChain, PullSwitchChainError, TriState};

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum ResistanceSolverError {
    PullSwitchChain(PullSwitchChainError),
    AdcChain(AdcChainError),
    Solve(SolveError),
}

/// Where one arm of one jack is wired and what it is driven through: the switch tap that
/// pulls it to a rail, the ADC input that reads it back, and the pull resistances in between.
///
/// These are the entries of the board's mapping table, so moving a jack to different
/// channels is a change to that table rather than to any measurement code.
#[derive(Debug, Clone, Copy)]
pub struct ArmConfig {
    pub switch_chip: usize,
    pub switch_tap: u8,
    pub adc_chip: usize,
    pub adc_channel: u8,
    /// Series resistance between the pull-up rail and the tap while pulled up, in kΩ.
    pub pull_up_resistance: f32,
    /// Series resistance between the pull-down rail and the tap while pulled down, in kΩ.
    pub pull_down_resistance: f32,
}

impl Default for ArmConfig {
    fn default() -> Self {
        ArmConfig {
            switch_chip: 0,
            switch_tap: 0,
            adc_chip: 0,
            adc_channel: 0,
            pull_up_resistance: SolverConfig::DEFAULT_PULL_RESISTANCE,
            pull_down_resistance: SolverConfig::DEFAULT_PULL_RESISTANCE,
        }
    }
}

/// Rail voltage each arm's tap reads while all three arms are driven to that rail, in V.
///
/// They are properties of the board - reference and ground, switch resistance - not of whatever
/// is plugged into a jack, so they hold until the hardware itself drifts and do not have to
/// be remeasured for every solve.
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct ArmRails {
    pub high: [f32; ARM_COUNT],
    pub low: [f32; ARM_COUNT],
}

/// One solved jack, together with the raw measurement it came from.
#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct SolveOutcome {
    pub resistances: ArmResistances,

    /// Tap voltage of every arm during the last pair measurement, in V.
    pub voltages: [f32; ARM_COUNT],

    /// How every arm was driven during that same measurement.
    pub pulls: [TriState; ARM_COUNT],
}

#[derive(Debug, Clone, Copy)]
pub struct ResistanceSolverConfig {
    pub solver: SolverConfig,

    /// Capacitance on every tap, in nF - the ADC input filter, mostly. Every floating tap
    /// charges through whatever is plugged into the jack, so together with that network's
    /// resistance this sets how long a pair measurement takes to settle.
    pub tap_capacitance: f32,

    /// Resistance between each tap and its capacitance, in kΩ - the ADC input filter's
    /// series resistor, which the network charges that capacitance through as well.
    pub tap_series_resistance: f32,

    /// Time constants of ((network + series resistance) x tap capacitance) to let a pair settle for
    /// after the pull switches change. The ADC averages over its whole conversion window,
    /// so a tap still moving while it converts corrupts that conversion rather than just
    /// delaying it. Using the network's total resistance overestimates every tap's actual
    /// time constant, so a small factor is already plenty.
    pub settle_time_constants: f32,

    /// Bounds on the settle time, the upper one also used while the network is unknown.
    pub min_settle_delay: Duration,
    pub max_settle_delay: Duration,

    /// Readings averaged into every rail voltage. The solver treats the rails as practically
    /// noise-free next to a single tap reading, and they are only measured once in a while,
    /// so averaging them costs little.
    pub rail_samples: u32,
}

impl Default for ResistanceSolverConfig {
    fn default() -> Self {
        Self {
            solver: SolverConfig::default(),
            tap_capacitance: 10.0,
            tap_series_resistance: 0.0,
            settle_time_constants: 2.0,
            min_settle_delay: Duration::from_millis(1),
            max_settle_delay: Duration::from_millis(400),
            rail_samples: 8,
        }
    }
}

/// Measures the resistor network behind one jack at a time, by driving pairs of its arms
/// to the rails through the pull switches and reading every tap back through the ADC
/// chain.
///
/// The decision of which pair to measure next, and how the measurements resolve into
/// resistances, belongs to [`SolveSequence`] in `expad-topology`; this type only carries
/// those decisions out on real hardware.
pub struct ResistanceSolver<'d, const N_SWITCHES: usize, const N_ADCS: usize> {
    config: ResistanceSolverConfig,
    switches: PullSwitchChain<'d, N_SWITCHES>,
    adcs: AdcChain<'d, N_ADCS>,
}

impl<'d, const N_SWITCHES: usize, const N_ADCS: usize> ResistanceSolver<'d, N_SWITCHES, N_ADCS> {
    pub fn new(
        config: ResistanceSolverConfig,
        switches: PullSwitchChain<'d, N_SWITCHES>,
        adcs: AdcChain<'d, N_ADCS>,
    ) -> Self {
        ResistanceSolver {
            config,
            switches,
            adcs,
        }
    }

    /// Measures every arm's own high and low rail, by driving all three arms to the same
    /// rail. No current can flow between arms at one potential, so every tap reads its own
    /// rail exactly, whatever is plugged in - and every tap charges through the jack's pull
    /// resistor rather than through the network, so it settles immediately.
    pub async fn measure_rails(
        &mut self,
        arms: &[ArmConfig; ARM_COUNT],
    ) -> Result<ArmRails, ResistanceSolverError> {
        let mut rails = ArmRails {
            high: [f32::NAN; ARM_COUNT],
            low: [f32::NAN; ARM_COUNT],
        };

        for (state, voltages) in [
            (TriState::High, &mut rails.high),
            (TriState::Low, &mut rails.low),
        ] {
            self.drive(arms, [state; ARM_COUNT], self.config.min_settle_delay)
                .await?;

            for (arm, voltage) in arms.iter().zip(voltages.iter_mut()) {
                *voltage = self.measure_average(arm).await?;
            }
        }

        self.release(arms)?;

        Ok(rails)
    }

    /// How long a pair measurement on a network of `expected_total` kΩ takes to settle, or
    /// the longest allowed if that is not known (`f32::NAN`).
    pub fn settle_delay(&self, expected_total: f32) -> Duration {
        if !expected_total.is_finite() {
            return self.config.max_settle_delay;
        }

        // kΩ x nF = µs
        let charging_resistance = expected_total + self.config.tap_series_resistance;
        let time_constant = charging_resistance * self.config.tap_capacitance;
        let settle_micros = self.config.settle_time_constants * time_constant;

        Duration::from_micros(settle_micros as u64)
            .clamp(self.config.min_settle_delay, self.config.max_settle_delay)
    }

    /// Works through the measurement sequence for one jack and resolves its arms. Leaves
    /// every arm floating again once it is done, so nothing stays driven into whatever is
    /// plugged in between solves.
    ///
    /// `expected_total` is the network's total resistance as last measured, in kΩ, which sets
    /// how long every pair is left to settle; `f32::NAN` if it is not known.
    pub async fn solve(
        &mut self,
        arms: &[ArmConfig; ARM_COUNT],
        rails: &ArmRails,
        expected_total: f32,
    ) -> Result<SolveOutcome, ResistanceSolverError> {
        let settle_delay = self.settle_delay(expected_total);
        let mut sequence = SolveSequence::new(self.config.solver);
        let mut voltages = [f32::NAN; ARM_COUNT];
        let mut pulls = [TriState::HiZ; ARM_COUNT];

        loop {
            match sequence.step() {
                SolveStep::MeasurePair { high, low } => {
                    let floating = ARM_COUNT - high - low;

                    pulls = [TriState::HiZ; ARM_COUNT];
                    pulls[high] = TriState::High;
                    pulls[low] = TriState::Low;
                    self.drive(arms, pulls, settle_delay).await?;

                    // The floating tap is read last, since it is the one that has to charge
                    // the ADC input's filter capacitor through the whole network behind it.
                    voltages[high] = self.measure_arm(&arms[high]).await?;
                    voltages[low] = self.measure_arm(&arms[low]).await?;
                    voltages[floating] = self.measure_arm(&arms[floating]).await?;
                    defmt::trace!("Pair {} high, {} low: taps {}V", high, low, voltages);

                    sequence.record(
                        PairMeasurement::from_voltages(
                            PairVoltages {
                                high: voltages[high],
                                low: voltages[low],
                                floating: voltages[floating],
                            },
                            ArmDrive {
                                rail_voltage: rails.high[high],
                                pull_resistance: arms[high].pull_up_resistance,
                            },
                            ArmDrive {
                                rail_voltage: rails.low[low],
                                pull_resistance: arms[low].pull_down_resistance,
                            },
                            &self.config.solver,
                        )
                        .map_err(ResistanceSolverError::Solve)?,
                    );
                }

                SolveStep::Finished(result) => {
                    self.release(arms)?;
                    let resistances = result.map_err(ResistanceSolverError::Solve)?;

                    return Ok(SolveOutcome {
                        resistances,
                        voltages,
                        pulls,
                    });
                }
            }
        }
    }

    /// Stops driving every arm of one jack.
    pub fn release(&mut self, arms: &[ArmConfig; ARM_COUNT]) -> Result<(), ResistanceSolverError> {
        self.set_outputs(arms, [TriState::HiZ; ARM_COUNT])
    }

    async fn measure_average(&mut self, arm: &ArmConfig) -> Result<f32, ResistanceSolverError> {
        let mut sum = 0.0;
        for _ in 0..self.config.rail_samples {
            sum += self.measure_arm(arm).await?;
        }

        Ok(sum / self.config.rail_samples as f32)
    }

    /// Applies one set of arm states and waits `settle_delay` for the taps to follow.
    async fn drive(
        &mut self,
        arms: &[ArmConfig; ARM_COUNT],
        states: [TriState; ARM_COUNT],
        settle_delay: Duration,
    ) -> Result<(), ResistanceSolverError> {
        self.set_outputs(arms, states)?;
        Timer::after(settle_delay).await;

        Ok(())
    }

    fn set_outputs(
        &mut self,
        arms: &[ArmConfig; ARM_COUNT],
        states: [TriState; ARM_COUNT],
    ) -> Result<(), ResistanceSolverError> {
        for (arm, state) in arms.iter().zip(states) {
            self.switches
                .set_output(arm.switch_chip, arm.switch_tap, state);
        }

        self.switches
            .update()
            .map_err(ResistanceSolverError::PullSwitchChain)
    }

    async fn measure_arm(&mut self, arm: &ArmConfig) -> Result<f32, ResistanceSolverError> {
        self.adcs
            .measure_channel(arm.adc_chip, arm.adc_channel)
            .await
            .map_err(ResistanceSolverError::AdcChain)
    }
}
