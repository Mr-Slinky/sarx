use std::fmt;
use std::ops::{BitOr, BitOrAssign};

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ComponentMask(u64);

impl ComponentMask {
    pub(crate) const CAPACITY: usize = u64::BITS as usize;

    pub(crate) const EMPTY: Self = Self(0);

    pub(crate) fn single(position: usize) -> Self {
        // Stops a shift past the last bit, which a release build would wrap round to a lower bit
        assert!(position < Self::CAPACITY, "the position must be below the capacity of the mask");
        Self(1 << position)
    }

    #[cfg(test)]
    pub(crate) fn from_positions(positions: &[usize]) -> Self {
        positions
            .iter()
            .fold(Self::EMPTY, |mask, &position| {
                mask | Self::single(position)
            })
    }

    pub(crate) fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for ComponentMask {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for ComponentMask {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl fmt::Binary for ComponentMask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.0, formatter)
    }
}
