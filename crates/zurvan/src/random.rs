pub(crate) struct Random(u32); // the generator's state, which is never 0

impl Random {
    pub(crate) fn new(seed: u32) -> Self {
        // Guards against a seed of 0, from which xorshift only ever produces 0
        assert_ne!(seed, 0, "the seed must not be 0");
        Self(seed)
    }

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