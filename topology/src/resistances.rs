/// Arms meeting at the star point of one jack.
pub const ARM_COUNT: usize = 3;

/// Arm of each contact of a TRS jack.
pub const TIP: usize = 0;
pub const RING: usize = 1;
pub const SLEEVE: usize = 2;

/// Solved resistance distribution of one star network.
///
/// Each arm's value is encoded the same way its resistance naturally is: `0.0` means
/// shorted to the star point, `f32::INFINITY` means isolated, `f32::NAN` means individually
/// unresolvable (only observable as part of a pair's combined resistance), and any other
/// finite value is that arm's fraction of [`total`](Self::total).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ArmResistances {
    /// Relative resistance of each arm, normalized so the three sum to 1.0.
    pub relative: [f32; ARM_COUNT],

    /// Total absolute resistance of all three arms in series, in kΩ.
    pub total: f32,
}

/// What a solved network is to a jack monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Network {
    /// No current between any two arms: nothing behind the plug conducts, or there is no plug.
    Disconnected,

    /// Exactly one arm sits at the star point - the wiper - and the other two carry the two
    /// halves of the track.
    Potentiometer { wiper: usize },

    /// Two arms sit at the star point, the lower-resistance one first: a potentiometer's
    /// wiper resting on one end of its track, so either of them could be the wiper.
    EndStop { candidates: [usize; 2] },

    /// A single element between tip and sleeve, a switch or a rheostat: with the ring shorted
    /// to the sleeve behind a mono plug, isolated behind a stereo one.
    TipSleeve { ring_shorted: bool },

    /// Any other network, such as a dual footswitch.
    Other,
}

impl ArmResistances {
    /// All three arms are isolated from each other ("nothing connected").
    pub const DISCONNECTED: Self = Self {
        relative: [f32::INFINITY; ARM_COUNT],
        total: f32::NAN,
    };

    /// Absolute resistance of `arm` in kΩ, or `f32::NAN` when it is unresolved.
    pub fn absolute(&self, arm: usize) -> f32 {
        self.relative[arm] * self.total
    }

    /// Classifies the network. An arm within `max_wiper_relative` of the star point passes
    /// as a wiper, a second one within `end_stop_relative` makes it an end stop, and two arms
    /// within `end_stop_relative` on ring and sleeve are what a mono plug's sleeve does to
    /// the ring.
    pub fn network(&self, max_wiper_relative: f32, end_stop_relative: f32) -> Network {
        if self.relative.iter().all(|relative| relative.is_infinite()) {
            return Network::Disconnected;
        }

        if let Some(potentiometer) = self.potentiometer(max_wiper_relative, end_stop_relative) {
            return potentiometer;
        }

        let [tip, ring, sleeve] = self.relative;
        let at_star_point = |relative: f32| relative <= end_stop_relative;
        if at_star_point(ring) && at_star_point(sleeve) {
            Network::TipSleeve { ring_shorted: true }
        } else if ring.is_infinite() && !(tip.is_infinite() && sleeve.is_infinite()) {
            Network::TipSleeve {
                ring_shorted: false,
            }
        } else {
            Network::Other
        }
    }

    /// Every arm resolved, the lowest one within `max_wiper_relative` of the star point and
    /// the highest one beyond it: a potentiometer, or one on its end stop if the middle arm
    /// is within `end_stop_relative` as well.
    fn potentiometer(&self, max_wiper_relative: f32, end_stop_relative: f32) -> Option<Network> {
        if !self.relative.iter().all(|relative| relative.is_finite()) {
            return None;
        }

        let mut arms = [0, 1, 2];
        arms.sort_unstable_by(|&left, &right| self.relative[left].total_cmp(&self.relative[right]));
        let [lowest, middle, highest] = arms;
        let at_star_point = |arm: usize| self.relative[arm] <= max_wiper_relative;
        if !at_star_point(lowest) || at_star_point(highest) {
            return None;
        }

        Some(if self.relative[middle] <= end_stop_relative {
            Network::EndStop {
                candidates: [lowest, middle],
            }
        } else {
            Network::Potentiometer { wiper: lowest }
        })
    }

    /// Position of `wiper` in `0.0..=1.0`, as the share of the track between its start (see
    /// [`track_ends`]) and the wiper - the same reading as driving the start low, the other
    /// end high and dividing the wiper voltage along. `None` if that track has no resistance
    /// or is unresolved.
    ///
    /// With the sleeve as the start, that is the TRS convention of both common wirings (wiper
    /// on the tip or on the ring, sleeve grounded): the value rises as the wiper leaves the
    /// sleeve end. Pedals wired the other way round are what the per-jack `inverted` setting
    /// exists for.
    pub fn position_with_wiper(&self, wiper: usize) -> Option<f32> {
        let (start, end) = track_ends(wiper);
        let track_sum = self.relative[start] + self.relative[end];
        // Also rejects a NaN sum, from an unresolved arm.
        if track_sum.is_nan() || track_sum <= 0.0 {
            return None;
        }

        Some(self.relative[start] / track_sum)
    }
}

/// Arm a pedal's track is grounded on in the TRS convention.
pub const GROUNDED_END: usize = SLEEVE;

/// The two arms that are not `wiper`, as the track's start and end: [`GROUNDED_END`] first
/// whenever it is one of them, so positions count up away from the sleeve, and otherwise the
/// lower-numbered one.
pub fn track_ends(wiper: usize) -> (usize, usize) {
    let mut ends = (0..ARM_COUNT).filter(|&arm| arm != wiper);
    let first = ends.next().unwrap_or(0);
    let second = ends.next().unwrap_or(0);

    if second == GROUNDED_END {
        (second, first)
    } else {
        (first, second)
    }
}
