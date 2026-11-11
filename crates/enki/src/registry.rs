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
/// The type is `pub`, since the public trait `ComponentSet` names it in the signature of `infos`.
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
    /// `find_or_assign_bit_with_repeated_type_returns_same_bit` in `world.rs` checks the repeat.
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
