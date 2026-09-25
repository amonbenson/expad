/// Arms meeting at the star point of one jack.
pub const ARM_COUNT: usize = 3;

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

impl ArmResistances {
    /// All three arms are isolated from each other ("nothing connected").
    pub const DISCONNECTED: Self = Self {
        relative: [f32::INFINITY; ARM_COUNT],
        total: f32::NAN,
    };

    /// Largest relative resistance an arm can have and still pass as a potentiometer's
    /// wiper, which sits at the star point and so contributes little more than its own
    /// contact and lead resistance.
    pub const DEFAULT_MAX_WIPER_RELATIVE: f32 = 0.1;

    /// Largest relative resistance a track end can have and still count as shorted to the
    /// star point by a wiper resting on it. Far below the wiper's own allowance: a pedal
    /// whose travel ends 9% short of its track (a common mechanical stop) is a potentiometer
    /// with a clear wiper, not an end stop.
    pub const END_STOP_RELATIVE: f32 = 0.02;

    /// Absolute resistance of `arm` in kΩ, or `f32::NAN` when it is unresolved.
    pub fn absolute(&self, arm: usize) -> f32 {
        self.relative[arm] * self.total
    }

    /// Position of the wiper in `0.0..=1.0`, if these arms form a usable potentiometer:
    /// one arm shorted to the star point (the wiper) and the other two carrying the two
    /// halves of the track. `None` for any other topology, including a disconnected or
    /// only partially resolved one.
    ///
    /// The position is the share of the track between its start (see [`track_ends`]) and the
    /// wiper - the same reading as driving the start low, the other end high and dividing the
    /// wiper voltage along. With arm 2 as the sleeve, that is the TRS convention of both common
    /// wirings (wiper on the tip or on the ring, sleeve grounded): the value rises as the
    /// wiper leaves the sleeve end. Pedals wired the other way round are what the per-jack
    /// `inverted` setting exists for.
    ///
    /// A wiper resting on a track end shorts it to the star point as well, which leaves two
    /// candidates for the wiper; this takes the one closer to the star point. See
    /// [`wiper_position_with_hint`](Self::wiper_position_with_hint).
    pub fn wiper_position(&self, max_wiper_relative: f32) -> Option<f32> {
        self.wiper_position_with_hint(max_wiper_relative, None)
    }

    /// Like [`wiper_position`](Self::wiper_position), but at an end stop takes
    /// `preferred_wiper` as the wiper whenever it is one of the two candidates.
    ///
    /// Only an end stop that shorts arms 1 and 2 (ring and sleeve) needs this: the two
    /// readings put the wiper on opposite ends of the track there. At every other end stop
    /// both candidates give the same position.
    pub fn wiper_position_with_hint(
        &self,
        max_wiper_relative: f32,
        preferred_wiper: Option<usize>,
    ) -> Option<f32> {
        let wiper = match self.potentiometer(max_wiper_relative)? {
            Potentiometer::Wiper(wiper) => wiper,
            Potentiometer::EndStop(candidates) => match preferred_wiper {
                Some(preferred) if candidates.contains(&preferred) => preferred,
                _ => candidates[0],
            },
        };

        self.position_with_wiper(wiper)
    }

    /// How these arms read as a potentiometer: every arm resolved, the lowest one within
    /// `max_wiper_relative` of the star point, and the highest one beyond it. The lowest is
    /// the wiper, unless the middle one is within [`END_STOP_RELATIVE`](Self::END_STOP_RELATIVE)
    /// as well - an end stop, where either of the two could be. `None` for any other topology.
    pub fn potentiometer(&self, max_wiper_relative: f32) -> Option<Potentiometer> {
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

        if self.relative[middle] <= Self::END_STOP_RELATIVE {
            Some(Potentiometer::EndStop([lowest, middle]))
        } else {
            Some(Potentiometer::Wiper(lowest))
        }
    }

    /// Position of the wiper as the share of the track between its start (see
    /// [`track_ends`]) and the wiper, if that track has any resistance at all.
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

/// How a solved network reads as a potentiometer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Potentiometer {
    /// Exactly this arm sits at the star point: the wiper.
    Wiper(usize),

    /// Two arms sit at the star point, the lower-resistance one first: the wiper resting on
    /// one end of the track. Either of them could be the wiper.
    EndStop([usize; 2]),
}

/// Arm a pedal's track is grounded on in the TRS convention: the sleeve, with the arms in
/// tip, ring, sleeve order.
pub const GROUNDED_END: usize = 2;

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
