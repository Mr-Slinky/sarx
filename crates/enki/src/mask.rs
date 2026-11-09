//! Defines the mask that records a set of component types, with one bit for each type.
//!
//! A world gives each component type a position, from 0 upwards, the first time that it meets the
//! type. For each type in its set, a [`ComponentMask`] sets the bit at that position. A table
//! stores a mask for the component types that it stores, and a query builds a mask for the types
//! that it asks for. The table then compares the two masks to answer whether it stores every type in the
//! query. The comparison does not visit any column.
//!
//! A mask has room for [`ComponentMask::CAPACITY`] component types. That number is also the limit
//! on the component types in one world. This module defines every operation on the bits. As a
//! result, a change to the limit changes this module alone.

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
/// The type is `pub` because the public trait `Bundle` names it in the signature of `push_into`.
/// The `mask` module is private, so code outside the crate still cannot write a path to the type.
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
pub struct ComponentMask(u64);

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
    /// An `other` of [`EMPTY`](Self::EMPTY) does not ask for any component types. The method
    /// therefore returns `true` for every `self`.
    pub(crate) fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` where `self` and `other` share at least one component type, and `false` where
    /// they share none.
    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns the mask with every component type in `self` apart from those in `other`.
    ///
    /// A type in `other` that `self` lacks leaves the result unchanged.
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
    /// Writes the bits of the mask in base 2.
    ///
    /// A format string requests this output with `{:b}` or `{:#b}`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.0, formatter)
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the base 2 text that a mask writes when its one set bit is at `position`
    fn binary_for_single(position: usize) -> String {
        format!("1{}", "0".repeat(position))
    }

    /// Checks that a mask has room for one component type per bit of a `u64`
    #[test]
    fn capacity_with_u64_mask_returns_64() {
        let capacity = ComponentMask::CAPACITY;

        assert_eq!(capacity, 64, "capacity of the mask");
    }

    /// Checks that the default mask is the empty mask, with no set bits
    #[test]
    fn default_with_no_data_returns_empty() {
        let mask = ComponentMask::default();

        let bits = format!("{mask:b}");

        assert_eq!(mask, ComponentMask::EMPTY, "default mask");
        assert_eq!(bits, "0", "bits of the default mask");
    }

    /// Checks that a single mask sets the bit at the given position alone, up to the last bit
    #[test]
    fn single_with_standard_and_extreme_positions_sets_one_bit() {
        let positions = [0, 1, 2, 31, 32, 62, 63];

        for position in positions {
            let mask = ComponentMask::single(position);

            let bits = format!("{mask:b}");

            assert_eq!(
                bits,
                binary_for_single(position),
                "bits for position {position}"
            );
        }
    }

    /// Checks that two different positions give two different masks
    #[test]
    fn single_with_different_positions_returns_different_masks() {
        let cases = [(0, 1), (0, 63), (31, 32), (62, 63)];

        for (first, second) in cases {
            let first_mask = ComponentMask::single(first);
            let second_mask = ComponentMask::single(second);

            assert_ne!(
                first_mask, second_mask,
                "masks for positions {first} and {second}"
            );
        }
    }

    /// Checks that the position one past the last bit is rejected, rather than wrapped round
    #[test]
    #[should_panic]
    fn single_with_position_at_capacity_panics() {
        let _ = ComponentMask::single(ComponentMask::CAPACITY);
    }

    /// Checks that the largest possible position is rejected
    #[test]
    #[should_panic]
    fn single_with_largest_position_panics() {
        let _ = ComponentMask::single(usize::MAX);
    }

    /// Checks that a mask built from several positions matches the single masks combined with `|`
    #[test]
    fn from_positions_with_standard_and_extreme_positions_returns_combined_mask() {
        let cases = [
            ("no positions", &[][..], ComponentMask::EMPTY),
            ("one position", &[3][..], ComponentMask::single(3)),
            (
                "two positions",
                &[0, 2][..],
                ComponentMask::single(0) | ComponentMask::single(2),
            ),
            (
                "first and last bits",
                &[0, 63][..],
                ComponentMask::single(0) | ComponentMask::single(63),
            ),
            ("repeated position", &[5, 5][..], ComponentMask::single(5)),
            (
                "positions out of order",
                &[7, 1][..],
                ComponentMask::single(1) | ComponentMask::single(7),
            ),
        ];

        for (name, positions, expected) in cases {
            let mask = ComponentMask::from_positions(positions);

            assert_eq!(mask, expected, "mask for {name}");
        }
    }

    /// Checks that one mask can have all 64 positions set
    #[test]
    fn from_positions_with_every_position_sets_every_bit() {
        let every_position: Vec<usize> = (0..ComponentMask::CAPACITY).collect();

        let mask = ComponentMask::from_positions(&every_position);
        let bits = format!("{mask:b}");

        assert_eq!(bits, "1".repeat(64), "bits of the full mask");
    }

    /// Checks that a position past the last bit is rejected, even after a valid position
    #[test]
    #[should_panic]
    fn from_positions_with_position_at_capacity_panics() {
        let _ = ComponentMask::from_positions(&[0, ComponentMask::CAPACITY]);
    }

    /// Checks that `contains_all` returns `true` only where `self` has every type in `other`
    #[test]
    fn contains_all_with_standard_and_extreme_masks_returns_expected() {
        let every_position: Vec<usize> = (0..ComponentMask::CAPACITY).collect();
        let full = ComponentMask::from_positions(&every_position);
        let table = ComponentMask::from_positions(&[0, 2, 63]);

        let cases = [
            ("same mask", table, table, true),
            (
                "subset",
                table,
                ComponentMask::from_positions(&[0, 63]),
                true,
            ),
            ("last bit", table, ComponentMask::single(63), true),
            ("empty other", table, ComponentMask::EMPTY, true),
            (
                "both empty",
                ComponentMask::EMPTY,
                ComponentMask::EMPTY,
                true,
            ),
            ("full self", full, table, true),
            ("missing type", table, ComponentMask::single(1), false),
            (
                "superset",
                table,
                ComponentMask::from_positions(&[0, 1, 2, 63]),
                false,
            ),
            (
                "empty self",
                ComponentMask::EMPTY,
                ComponentMask::single(0),
                false,
            ),
            ("full other", table, full, false),
        ];

        for (name, mask, other, expected) in cases {
            let result = mask.contains_all(other);

            assert_eq!(result, expected, "contains_all for {name}");
        }
    }

    /// Checks that `intersects` returns `true` only where the two masks share a type
    #[test]
    fn intersects_with_standard_and_extreme_masks_returns_expected() {
        let table = ComponentMask::from_positions(&[0, 2, 63]);

        let cases = [
            ("same mask", table, table, true),
            (
                "one shared type",
                table,
                ComponentMask::from_positions(&[1, 2]),
                true,
            ),
            ("last bit shared", table, ComponentMask::single(63), true),
            (
                "no shared type",
                table,
                ComponentMask::from_positions(&[1, 3, 62]),
                false,
            ),
            ("empty other", table, ComponentMask::EMPTY, false),
            ("empty self", ComponentMask::EMPTY, table, false),
            (
                "both empty",
                ComponentMask::EMPTY,
                ComponentMask::EMPTY,
                false,
            ),
        ];

        for (name, mask, other, expected) in cases {
            let result = mask.intersects(other);

            assert_eq!(result, expected, "intersects for {name}");
        }
    }

    /// Checks that `without` removes the types in `other`, and ignores those that `self` lacks
    #[test]
    fn without_with_standard_and_extreme_masks_returns_remaining_types() {
        let table = ComponentMask::from_positions(&[0, 2, 63]);

        let cases = [
            (
                "one type",
                table,
                ComponentMask::single(2),
                ComponentMask::from_positions(&[0, 63]),
            ),
            (
                "last bit",
                table,
                ComponentMask::single(63),
                ComponentMask::from_positions(&[0, 2]),
            ),
            ("every type", table, table, ComponentMask::EMPTY),
            ("empty other", table, ComponentMask::EMPTY, table),
            ("absent type", table, ComponentMask::single(1), table),
            (
                "present and absent types",
                table,
                ComponentMask::from_positions(&[1, 2]),
                ComponentMask::from_positions(&[0, 63]),
            ),
            (
                "empty self",
                ComponentMask::EMPTY,
                table,
                ComponentMask::EMPTY,
            ),
        ];

        for (name, mask, other, expected) in cases {
            let result = mask.without(other);

            assert_eq!(result, expected, "without for {name}");
        }
    }

    /// Checks that `|` returns every type in either mask
    #[test]
    fn bitor_with_standard_and_extreme_masks_returns_union() {
        let table = ComponentMask::from_positions(&[0, 2]);

        let cases = [
            (
                "disjoint masks",
                table,
                ComponentMask::single(63),
                ComponentMask::from_positions(&[0, 2, 63]),
            ),
            (
                "overlapping masks",
                table,
                ComponentMask::from_positions(&[2, 3]),
                ComponentMask::from_positions(&[0, 2, 3]),
            ),
            ("same mask", table, table, table),
            ("empty other", table, ComponentMask::EMPTY, table),
            ("empty self", ComponentMask::EMPTY, table, table),
            (
                "both empty",
                ComponentMask::EMPTY,
                ComponentMask::EMPTY,
                ComponentMask::EMPTY,
            ),
        ];

        for (name, mask, other, expected) in cases {
            let result = mask | other;

            assert_eq!(result, expected, "union for {name}");
        }
    }

    /// Checks that `|=` adds every type in `other` to the mask
    #[test]
    fn bitor_assign_with_standard_and_extreme_masks_adds_types() {
        let table = ComponentMask::from_positions(&[0, 2]);

        let cases = [
            (
                "disjoint masks",
                table,
                ComponentMask::single(63),
                ComponentMask::from_positions(&[0, 2, 63]),
            ),
            (
                "overlapping masks",
                table,
                ComponentMask::from_positions(&[2, 3]),
                ComponentMask::from_positions(&[0, 2, 3]),
            ),
            ("same mask", table, table, table),
            ("empty other", table, ComponentMask::EMPTY, table),
            ("empty self", ComponentMask::EMPTY, table, table),
        ];

        for (name, mut mask, other, expected) in cases {
            mask |= other;

            assert_eq!(mask, expected, "mask after adding for {name}");
        }
    }

    /// Checks that `{:b}` writes the bits of the mask in base 2, with no leading zeros
    #[test]
    fn fmt_binary_with_standard_and_extreme_masks_writes_bits() {
        let cases = [
            ("empty", ComponentMask::EMPTY, "0".to_string()),
            ("first bit", ComponentMask::single(0), "1".to_string()),
            (
                "two bits",
                ComponentMask::from_positions(&[0, 2]),
                "101".to_string(),
            ),
            ("last bit", ComponentMask::single(63), binary_for_single(63)),
        ];

        for (name, mask, expected) in cases {
            let text = format!("{mask:b}");

            assert_eq!(text, expected, "binary text for {name}");
        }
    }

    /// Checks that `{:#b}` writes the bits of the mask after the `0b` prefix
    #[test]
    fn fmt_binary_with_alternate_flag_writes_prefixed_bits() {
        let mask = ComponentMask::from_positions(&[0, 2]);

        let text = format!("{mask:#b}");

        assert_eq!(text, "0b101", "prefixed binary text");
    }
}
