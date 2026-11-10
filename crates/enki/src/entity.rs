//! Defines the handle that refers to one entity in a world.
//!
//! The world documentation sets out how a world uses the index and the version in each handle.
//!
//! The example on [`Entity`] spawns an entity, copies its handle, and despawns the entity.

/// Refers to one entity in a [`World`](crate::World).
///
/// A handle stores an index and a version. The index selects the slot that the entity occupies in
/// the world. The version tells apart the entities that use the same slot at different times. A
/// handle therefore refers to its entity until a caller despawns that entity. After that, every
/// call with the handle does not find an entity.
///
/// A handle is `Copy`, meaning a caller can keep and pass as many copies as it needs. Every copy
/// refers to the same entity.
///
/// # Examples
///
/// The example spawns an entity, keeps a copy of its handle, then despawns the entity through the
/// original handle.
///
/// ```
/// use enki::World;
///
/// // Spawns an entity, and copies its handle
/// let mut world = World::new();
/// let player = world.spawn_empty();
/// let copy = player;
///
/// // Despawns the entity, after which the copy does not find an entity either
/// world.despawn(player);
/// assert!(!world.despawn(copy));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Stores the position of the slot that the entity occupies in the world.
    ///
    /// The example on [`Entity::new`] reads the index back through [`Entity::index`].
    index: u32,
    /// Stores the version that the slot had when the world created this entity.
    ///
    /// The example on [`Entity::new`] reads the version back through [`Entity::version`].
    version: u32,
}

impl Entity {
    /// Returns a handle with the slot position `index` and the version `version`.
    ///
    /// # Examples
    ///
    /// `Entity::new(5, 2)` returns a handle whose `index` returns 5 and whose `version` returns 2.
    /// The test `new_with_standard_and_extreme_values_returns_same_index_and_version` checks the
    /// extreme values.
    pub(crate) fn new(index: u32, version: u32) -> Self {
        Self { index, version }
    }

    /// Returns the position of the slot that the entity occupies, as a `usize`, the type that
    /// indexes a `Vec`.
    ///
    /// The example on [`Entity::new`] creates a handle for slot 5, and this method returns `5`.
    pub(crate) fn index(self) -> usize {
        self.index as usize
    }

    /// Returns the version that the slot had when the world created this entity.
    ///
    /// The example on [`Entity::new`] creates a handle at version 2, and this method returns `2`.
    pub(crate) fn version(self) -> u32 {
        self.version
    }
}

// ========================================================================================== \
//                                           Tests                                            \
// ========================================================================================== \

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    /// Returns the hash that the standard hasher computes for `entity`.
    fn hash_of(entity: Entity) -> u64 {
        let mut hasher = DefaultHasher::new();
        entity.hash(&mut hasher);
        hasher.finish()
    }

    /// Checks that a handle returns the index and the version that it was created with, at both ends
    /// of the `u32` range
    #[test]
    fn new_with_standard_and_extreme_values_returns_same_index_and_version() {
        // Pairs each index with a version, from ordinary values to the limits of `u32`
        let cases = [
            ("ordinary", 5, 3),
            ("both zero", 0, 0),
            ("both maximum", u32::MAX, u32::MAX),
            ("zero index, maximum version", 0, u32::MAX),
            ("maximum index, zero version", u32::MAX, 0),
        ];

        let handles =
            cases.map(|(name, index, version)| (name, index, version, Entity::new(index, version)));

        for (name, index, version, entity) in handles {
            assert_eq!(entity.index(), index as usize, "index of {name}");
            assert_eq!(entity.version(), version, "version of {name}");
        }
    }

    /// Checks that two handles are equal only when both their index and their version match
    #[test]
    fn eq_with_matching_and_differing_fields_returns_expected_result() {
        // Pairs two handles with whether they should be equal
        let cases = [
            (
                "same index and version",
                Entity::new(4, 2),
                Entity::new(4, 2),
                true,
            ),
            (
                "different version",
                Entity::new(4, 2),
                Entity::new(4, 3),
                false,
            ),
            (
                "different index",
                Entity::new(4, 2),
                Entity::new(5, 2),
                false,
            ),
            (
                "index and version swapped",
                Entity::new(4, 2),
                Entity::new(2, 4),
                false,
            ),
            (
                "both fields differ",
                Entity::new(0, 0),
                Entity::new(u32::MAX, u32::MAX),
                false,
            ),
        ];

        let results = cases.map(|(name, left, right, expected)| (name, left == right, expected));

        for (name, equal, expected) in results {
            assert_eq!(equal, expected, "equality for {name}");
        }
    }

    /// Checks that a copy of a handle stays equal to the original, since every copy refers to the
    /// same entity
    #[test]
    fn copy_with_standard_handle_returns_equal_handle() {
        let original = Entity::new(7, 9);

        let copy = original;

        assert_eq!(copy, original);
    }

    /// Checks that two equal handles produce the same hash, as `Hash` must agree with `Eq`
    #[test]
    fn hash_with_equal_handles_returns_same_hash() {
        let first = Entity::new(12, 1);
        let second = Entity::new(12, 1);

        let first_hash = hash_of(first);
        let second_hash = hash_of(second);

        assert_eq!(first_hash, second_hash);
    }

    /// Checks that a hash set keeps apart handles that share a slot but differ in version
    #[test]
    fn hash_with_same_index_different_versions_returns_distinct_set_entries() {
        let mut set = HashSet::new();

        set.insert(Entity::new(3, 0));
        set.insert(Entity::new(3, 1));
        set.insert(Entity::new(3, 0));

        assert_eq!(set.len(), 2);
    }
}
