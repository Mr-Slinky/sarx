//! Defines the mask that records a set of component types, with one bit for each type.
//!
//! A world gives each component type a position, from 0 upwards, the first time that it meets the
//! type. A [`ComponentMask`] sets the bit at the position of each type in its set. A table stores
//! the mask of its own set of types, and a query builds the mask of the types that it asks for.
//! Comparing the two masks then answers whether the table stores every type in the query, without
//! a visit to any column.
//!
//! A mask has room for [`ComponentMask::CAPACITY`] component types. That number is also the limit
//! on the component types in one world. Every operation on the bits lives in this file, meaning a
//! change to the limit changes this file alone.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Records a set of component types, as one set bit at the position of each type.
///
/// A caller builds the mask for one type with [`single`](Self::single), and combines masks with
/// `|`. [`EMPTY`](Self::EMPTY) is the mask for the set with no types.
///
/// A test can also build a mask for several types in one call, with `from_positions`. That method
/// exists in test builds alone.
///
/// # Examples
///
/// The example builds a mask for two types, then checks which types it contains.
///
/// ```ignore
/// // Builds the masks for the types at positions 0 and 2, then the mask for both
/// let position = ComponentMask::single(0);
/// let health = ComponentMask::single(2);
/// let table = position | health;
///
/// // Checks that the combined mask contains `position`, and lacks the type at position 1
/// assert!(table.contains_all(position));
/// assert!(!table.contains_all(ComponentMask::single(1)));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ComponentMask(u64);

impl ComponentMask {
    /// Defines the number of component types that a mask has room for.
    pub(crate) const CAPACITY: usize = u64::BITS as usize;

    /// Defines the mask for the set with no component types.
    pub(crate) const EMPTY: Self = Self(0);

    /// Returns the mask with one set bit, at `position`.
    ///
    /// # Panics
    ///
    /// Panics where `position` is not below [`CAPACITY`](Self::CAPACITY).
    pub(crate) fn single(position: usize) -> Self {
        // Stops a shift past the last bit, which a release build would wrap round to a lower bit
        assert!(
            position < Self::CAPACITY,
            "the position must be below the capacity of the mask"
        );
        Self(1 << position)
    }

    /// Returns the mask with a set bit at each entry of `positions`, for tests to build a mask in one
    /// line.
    ///
    /// # Panics
    ///
    /// Panics where an entry is not below [`CAPACITY`](Self::CAPACITY).
    #[cfg(test)]
    pub(crate) fn from_positions(positions: &[usize]) -> Self {
        positions
            .iter()
            .fold(Self::EMPTY, |mask, &position| mask | Self::single(position))
    }

    /// Returns `true` where `self` contains every component type in `other`, and `false` where it
    /// lacks at least one.
    ///
    /// An `other` of [`EMPTY`](Self::EMPTY) does not ask for any component types, and the method
    /// then returns `true` for every `self`.
    pub(crate) fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` where `self` and `other` share at least one component type, and `false` where
    /// they share none.
    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub(crate) fn without(mut self, other: Self) -> Self {
        self.0 &= !other.0;
        self
    }
}

impl BitOr for ComponentMask {
    type Output = Self;

    /// Returns the mask with every component type that `self` or `other` contains.
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for ComponentMask {
    /// Adds every component type in `other` to `self`.
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl fmt::Binary for ComponentMask {
    /// Writes the bits of the mask in base 2, which a `{:b}` or `{:#b}` in a format string requests.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.0, formatter)
    }
}
