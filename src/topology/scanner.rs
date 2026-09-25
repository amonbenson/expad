use embassy_time::{Duration, Instant, Timer};
use expad_topology::{CONTACT_COUNT, Drive, JackMonitor, JackReport, MonitorConfig};

use crate::hal::adc::{AdcChain, AdcChainError};
use crate::hal::buf::{PullSwitchChain, PullSwitchChainError, TriState};

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum ScannerError {
    PullSwitchChain(PullSwitchChainError),
    AdcChain(AdcChainError),
}

/// Where one jack's contacts are wired, in contact order: the three arms, then the tip
/// switch. These are the entries of the board's mapping table, so moving a jack to different
/// channels is a change to that table rather than to any measurement code.
#[derive(Debug, Clone, Copy)]
pub struct JackConfig {
    pub switch_chip: usize,
    pub switch_taps: [u8; CONTACT_COUNT],
    pub adc_chip: usize,
    pub adc_channels: [u8; CONTACT_COUNT],
}

/// How long the taps are left to settle after a jack's drives change.
#[derive(Debug, Clone, Copy)]
pub struct SettleConfig {
    /// Capacitance on every tap, in nF - the ADC input filter's.
    pub tap_capacitance: f32,

    /// Resistance between each contact and its capacitance, in kΩ - the ADC input filter's
    /// series resistor, which the network charges that capacitance through as well.
    pub tap_series_resistance: f32,

    /// Time constants of ((network + series resistance) x tap capacitance) to settle for.
    /// The network's total resistance overestimates every tap's actual time constant, so a
    /// small factor is already plenty.
    pub time_constants: f32,

    /// Bounds on the settle time, the upper one also used while the network is unknown.
    pub min_delay: Duration,
    pub max_delay: Duration,
}

impl Default for SettleConfig {
    fn default() -> Self {
        Self {
            tap_capacitance: 10.0,
            tap_series_resistance: 0.0,
            time_constants: 2.0,
            min_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(400),
        }
    }
}

impl SettleConfig {
    /// How long taps charging through `resistance` kΩ take to settle, or the longest allowed
    /// if that is not known (`f32::NAN`).
    pub fn delay(&self, resistance: f32) -> Duration {
        if !resistance.is_finite() {
            return self.max_delay;
        }

        // kΩ x nF = µs
        let time_constant = (resistance + self.tap_series_resistance) * self.tap_capacitance;
        let settle_micros = self.time_constants * time_constant;

        Duration::from_micros(settle_micros as u64).clamp(self.min_delay, self.max_delay)
    }
}

/// Runs one [`JackMonitor`] per jack on the real pull switches and ADC chain.
///
/// Every [`step`](Self::step) takes one reading on each ADC at once, for the jack on it whose
/// reading has been due longest. A jack's drives stay applied while another jack on the same
/// ADC is read, so alternating between two tracked pedals costs no tap settling, only the
/// ADC's own.
pub struct JackScanner<'d, const N_SWITCHES: usize, const N_ADCS: usize, const N_JACKS: usize> {
    switches: PullSwitchChain<'d, N_SWITCHES>,
    adcs: AdcChain<'d, N_ADCS>,
    settle: SettleConfig,
    jacks: [JackConfig; N_JACKS],
    monitors: [JackMonitor; N_JACKS],
    applied: [[Drive; CONTACT_COUNT]; N_JACKS],
    /// When each jack was last read, counted in readings, so due jacks on one ADC take turns.
    last_served: [u64; N_JACKS],
    readings: u64,
}

impl<'d, const N_SWITCHES: usize, const N_ADCS: usize, const N_JACKS: usize>
    JackScanner<'d, N_SWITCHES, N_ADCS, N_JACKS>
{
    /// Takes over `switches`, which must have every switch open, and `adcs`, which must be
    /// initialized.
    pub fn new(
        switches: PullSwitchChain<'d, N_SWITCHES>,
        adcs: AdcChain<'d, N_ADCS>,
        settle: SettleConfig,
        jacks: [JackConfig; N_JACKS],
        monitor: MonitorConfig,
    ) -> Self {
        Self {
            switches,
            adcs,
            settle,
            jacks,
            monitors: core::array::from_fn(|_| JackMonitor::new(monitor)),
            applied: [[Drive::Floating; CONTACT_COUNT]; N_JACKS],
            last_served: [0; N_JACKS],
            readings: 0,
        }
    }

    pub fn report(&self, jack: usize) -> &JackReport {
        self.monitors[jack].report()
    }

    /// Sets which arm of `jack` is the wiper when an end stop leaves it ambiguous.
    pub fn set_preferred_wiper(&mut self, jack: usize, preferred_wiper: Option<usize>) {
        self.monitors[jack].set_preferred_wiper(preferred_wiper);
    }

    /// Takes one reading on every ADC that has a jack due, all ADCs converting at once, and
    /// returns which jacks' reports changed. Waits for the next due reading if there is none
    /// yet.
    pub async fn step(&mut self) -> Result<[bool; N_JACKS], ScannerError> {
        let now = Instant::now().as_millis();
        let mut chosen = [None::<usize>; N_ADCS];
        for jack in 0..N_JACKS {
            if self.monitors[jack].due_at() > now {
                continue;
            }

            let slot = &mut chosen[self.jacks[jack].adc_chip];
            match *slot {
                Some(other) if self.last_served[other] <= self.last_served[jack] => {}
                _ => *slot = Some(jack),
            }
        }

        if chosen.iter().all(Option::is_none) {
            let next_due = self.monitors.iter().map(JackMonitor::due_at).min();
            Timer::at(Instant::from_millis(next_due.unwrap_or(now + 1))).await;
            return Ok([false; N_JACKS]);
        }

        let mut channels = [None; N_ADCS];
        let mut settle_delay = None::<Duration>;
        for (channel, jack) in channels.iter_mut().zip(chosen) {
            let Some(jack) = jack else {
                continue;
            };

            let reading = self.monitors[jack].reading();
            let wiring = self.jacks[jack];
            *channel = Some(wiring.adc_channels[reading.contact]);

            if reading.drives != self.applied[jack] {
                for (tap, drive) in wiring.switch_taps.iter().zip(reading.drives) {
                    self.switches
                        .set_output(wiring.switch_chip, *tap, tri_state(drive));
                }
                self.applied[jack] = reading.drives;

                let delay = self.settle.delay(reading.settle_resistance);
                settle_delay = Some(settle_delay.map_or(delay, |other| other.max(delay)));
            }
        }

        if let Some(delay) = settle_delay {
            self.switches
                .update()
                .map_err(ScannerError::PullSwitchChain)?;
            Timer::after(delay).await;
        }

        let voltages = self
            .adcs
            .measure_parallel(channels)
            .await
            .map_err(ScannerError::AdcChain)?;

        let now = Instant::now().as_millis();
        let mut changed = [false; N_JACKS];
        for (jack, voltage) in chosen.into_iter().zip(voltages) {
            let (Some(jack), Some(voltage)) = (jack, voltage) else {
                continue;
            };

            changed[jack] = self.monitors[jack].record(voltage, now);
            self.readings += 1;
            self.last_served[jack] = self.readings;
        }

        Ok(changed)
    }
}

fn tri_state(drive: Drive) -> TriState {
    match drive {
        Drive::High => TriState::High,
        Drive::Low => TriState::Low,
        Drive::Floating => TriState::HiZ,
    }
}
