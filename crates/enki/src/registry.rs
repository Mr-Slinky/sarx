//! Defines [`ComponentRegistry`], which gives each component type in a world its bit.
//!
//! The registry gives each component type the next free bit, the first time that it meets the
//! type. The first type takes bit 0, the second takes bit 1, and the order continues up to the
//! last bit that a [`ComponentMask`] has. The world builds the mask of each table and each query
//! from these bits.
//!
//! The example on [`ComponentRegistry`] gives two types their bits.

use crate::mask::ComponentMask;
use std::any::TypeId;
use std::collections::HashMap;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Maps each component type that a world has met to its bit.
///
/// The type is `pub`, since the public trait `ComponentSet` uses it in the signature of `infos`.
/// The `registry` module is private, meaning code outside the crate cannot write a path to the
/// type. Only `enki` can create a registry, since [`new`](Self::new) is `pub(crate)`.
///
/// # Examples
///
/// The example gives `Position` and `Health` their bits, then asks for `Position` again.
///
/// ```ignore
/// let mut registry = ComponentRegistry::new();
///
/// // Gives `Position` bit 0 and `Health` bit 1, in the order that the registry meets them
/// let position = registry.find_or_assign_bit(TypeId::of::<Position>());
/// let health = registry.find_or_assign_bit(TypeId::of::<Health>());
///
/// // Returns the bit that `Position` already has
/// assert_eq!(registry.find_bit(TypeId::of::<Position>()), Some(position));
/// assert_eq!(health, ComponentMask::single(1));
/// ```
pub struct ComponentRegistry {
    /// Maps the `TypeId` of each component type to its bit.
    ///
    /// For example, a registry that met `Position` first maps `TypeId::of::<Position>()` to the
    /// mask with bit 0 set.
    bits: HashMap<TypeId, ComponentMask>,
}

impl ComponentRegistry {
    /// Returns a registry with no component types, and every bit free.
    ///
    /// The example on [`ComponentRegistry`] shows a call.
    pub(crate) fn new() -> Self {
        Self {
            bits: HashMap::new(),
        }
    }

    /// Returns the bit for the type that `type_id` identifies. Gives the type the next free bit
    /// where it has none yet.
    ///
    /// # Panics
    ///
    /// Panics where the type has no bit yet, and the registry already has
    /// [`ComponentMask::CAPACITY`] component types.
    ///
    /// # Examples
    ///
    /// In a new registry, the first type that the method meets takes bit 0, and the second takes
    /// bit 1. A later call for the first type returns bit 0 again. The test
    /// `find_or_assign_bit_with_repeated_type_returns_same_bit` checks the repeat.
    pub(crate) fn find_or_assign_bit(&mut self, type_id: TypeId) -> ComponentMask {
        // Counts the bits handed out so far, which is also the position of the next free bit
        let next_position = self.bits.len();
        // Returns the type's bit, first giving it the next free bit where it has none yet
        *self.bits.entry(type_id).or_insert_with(|| {
            // Panics where every bit is taken, since a mask has no bit left for another type
            let capacity = ComponentMask::CAPACITY;
            assert!(
                next_position < capacity,
                "a world has at most {capacity} component types"
            );
            ComponentMask::single(next_position)
        })
    }

    /// Returns the bit for the type that `type_id` identifies, or `None` where the type has no bit
    /// yet.
    ///
    /// Unlike [`find_or_assign_bit`](Self::find_or_assign_bit), the method never assigns a bit.
    ///
    /// # Examples
    ///
    /// In a registry that has met `Position` alone, the method returns bit 0 for `Position`, and
    /// `None` for `Health`.
    pub(crate) fn find_bit(&self, type_id: TypeId) -> Option<ComponentMask> {
        self.bits.get(&type_id).copied()
    }

    /// Returns the number of component types that have a bit.
    ///
    /// The example on [`ComponentRegistry`] meets two types, after which the method returns 2.
    pub(crate) fn count_types(&self) -> usize {
        self.bits.len()
    }
}

// ========================================================================================== \
//                                           Tests                                            \
// ========================================================================================== \

/// Builds an array with the `TypeId` of each array type `[u8; N]`, for every `N` listed.
#[cfg(test)]
macro_rules! array_type_ids {
    ($($length:literal)*) => { [$(TypeId::of::<[u8; $length]>()),*] };
}

/// Returns 64 distinct `TypeId`s, enough to take every bit that a registry can hand out.
///
/// The tests in `registry.rs`, `set.rs` and `world.rs` fill a registry with these types.
#[cfg(test)]
pub(crate) fn build_64_type_ids() -> [TypeId; 64] {
    array_type_ids!(
         0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
        16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
        32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
        48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::array;

    struct Position;
    struct Health;

    /// Checks that 64 component types take every bit of the mask, one bit each
    #[test]
    fn find_or_assign_bit_with_64_types_assigns_every_bit_once() {
        let mut registry = ComponentRegistry::new();
        let expected: [ComponentMask; 64] = array::from_fn(ComponentMask::single);

        let bits = build_64_type_ids().map(|type_id| registry.find_or_assign_bit(type_id));

        assert_eq!(bits, expected, "one bit each, in order of first sight");
    }

    /// Checks that a type seen before keeps its first bit, and does not take a new one
    #[test]
    fn find_or_assign_bit_with_repeated_type_returns_same_bit() {
        let mut registry = ComponentRegistry::new();
        let first = registry.find_or_assign_bit(TypeId::of::<Position>());
        let other = registry.find_or_assign_bit(TypeId::of::<Health>());
        let expected = [ComponentMask::single(0), ComponentMask::single(1)];

        let repeat = registry.find_or_assign_bit(TypeId::of::<Position>());

        assert_eq!(repeat, first, "bit for the repeated type");
        assert_eq!([first, other], expected, "bits in order of first sight");
        assert_eq!(registry.count_types(), 2, "component types");
    }

    /// Checks that a type seen before keeps its bit, even once every bit is taken
    #[test]
    fn find_or_assign_bit_with_full_registry_and_known_type_returns_its_bit() {
        let mut registry = ComponentRegistry::new();
        let type_ids = build_64_type_ids();
        for type_id in type_ids {
            registry.find_or_assign_bit(type_id);
        }

        let last = registry.find_or_assign_bit(type_ids[63]);

        assert_eq!(last, ComponentMask::single(63), "bit of the last type");
        assert_eq!(registry.count_types(), 64, "component types");
    }

    /// Checks that `find_or_assign_bit` panics on a 65th type, since a mask has room for 64 bits
    #[test]
    #[should_panic]
    fn find_or_assign_bit_with_65th_type_panics() {
        let mut registry = ComponentRegistry::new();
        for type_id in build_64_type_ids() {
            registry.find_or_assign_bit(type_id);
        }

        registry.find_or_assign_bit(TypeId::of::<Health>());
    }

    /// Checks that `find_bit` returns the bit of a type met before, and `None` for any other,
    /// without assigning a bit
    #[test]
    fn find_bit_with_known_and_unknown_types_returns_bit_only_for_known() {
        let mut registry = ComponentRegistry::new();
        registry.find_or_assign_bit(TypeId::of::<Position>());

        let known = registry.find_bit(TypeId::of::<Position>());
        let unknown = registry.find_bit(TypeId::of::<Health>());

        assert_eq!(
            known,
            Some(ComponentMask::single(0)),
            "bit of the known type"
        );
        assert_eq!(unknown, None, "bit of the unknown type");
        assert_eq!(registry.count_types(), 1, "component types");
    }

    /// Checks that `count_types` counts each type once, from an empty registry to a full one
    #[test]
    fn count_types_with_varying_types_returns_number_of_types() {
        let type_ids = build_64_type_ids();
        let cases = [0, 1, 5, 64];

        let found = cases.map(|count| {
            let mut registry = ComponentRegistry::new();
            for type_id in &type_ids[..count] {
                registry.find_or_assign_bit(*type_id);
                registry.find_or_assign_bit(*type_id);
            }
            (count, registry.count_types())
        });

        for (count, types) in found {
            assert_eq!(types, count, "component types after {count} distinct types");
        }
    }
}
