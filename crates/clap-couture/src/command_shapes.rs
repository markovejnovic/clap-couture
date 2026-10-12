//! Seeded choices for generating clap commands to check couture against: the same seed always
//! makes the same choices.

/// A deterministic stream of choices: Knuth's MMIX linear congruential generator.
pub(crate) struct Choices(u64);

impl Choices {
    /// A number below `bound`, or 0 when `bound` is 0.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let high_bits = usize::try_from(self.0 >> 33u32).unwrap_or_default();
        high_bits.checked_rem(bound).unwrap_or_default()
    }

    pub(crate) const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// True once in `odds` times.
    pub(crate) fn one_in(&mut self, odds: usize) -> bool {
        self.below(odds) == 0
    }
}
