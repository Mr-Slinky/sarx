//! Generates the pseudo-random numbers that the simulation picks idle poses with.

/// Generates pseudo-random numbers by xorshift. Two generators built from the same seed produce
/// the same sequence of numbers.
pub(crate) struct Random(u32); // the generator's state, which is never 0

impl Random {
    /// Returns a generator that starts from `seed`.
    ///
    /// # Panics
    ///
    /// Panics if `seed` is 0.
    pub(crate) fn new(seed: u32) -> Self {
        // Guards against a seed of 0, from which xorshift only ever produces 0
        assert_ne!(seed, 0, "the seed must not be 0");
        Self(seed)
    }

    /// Returns the next number in the sequence, reduced to the range from 0 up to, but not
    /// including, `bound`.
    ///
    /// # Panics
    ///
    /// Panics if `bound` is 0.
    pub(crate) fn pick_below(&mut self, bound: usize) -> usize {
        // Guards the remainder below, since a bound of 0 has no number below it
        assert!(bound > 0, "the bound must be above 0");
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        self.0 = state;
        (state as usize) % bound
    }

}