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

    /// Absolute resistance of `arm` in kΩ, or `f32::NAN` when it is unresolved.
    pub fn absolute(&self, arm: usize) -> f32 {
        self.relative[arm] * self.total
    }

    /// Position of the wiper in `0.0..=1.0`, if these arms form a usable potentiometer:
    /// one arm shorted to the star point (the wiper) and the other two carrying the two
    /// halves of the track. `None` for any other topology, including a disconnected or
    /// only partially resolved one.
    ///
    /// The position is the share of the track between the lower-numbered end and the wiper,
    /// so it counts up as the wiper travels towards the higher-numbered end - the same
    /// reading as driving the lower-numbered end low, the other one high and dividing the
    /// wiper voltage along. Which physical direction that is depends on how the pedal is
    /// wired, which is what the per-jack `inverted` setting exists for.
    pub fn wiper_position(&self, max_wiper_relative: f32) -> Option<f32> {
        let wiper = self.wiper(max_wiper_relative)?;

        let mut track = (0..ARM_COUNT).filter(|&arm| arm != wiper);
        let (start, end) = (track.next()?, track.next()?);

        let track_sum = self.relative[start] + self.relative[end];
        if track_sum <= 0.0 {
            return None;
        }

        Some(self.relative[start] / track_sum)
    }

    /// Index of the arm that behaves like a potentiometer's wiper: every arm is resolved,
    /// and exactly this one is (near enough) shorted to the star point.
    fn wiper(&self, max_wiper_relative: f32) -> Option<usize> {
        if !self.relative.iter().all(|relative| relative.is_finite()) {
            return None;
        }

        let (wiper, &lowest) = self
            .relative
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.total_cmp(right))?;

        (lowest <= max_wiper_relative).then_some(wiper)
    }
}
