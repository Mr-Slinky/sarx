//! Defines the handle that refers to one entity in a world.
//!
//! The world documentation sets out how a world uses the index and the version in each handle.

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
/// let player = world.spawn_empty_ent();
/// let copy = player;
///
/// // Despawns the entity, after which the copy does not find an entity either
/// world.despawn_ent(player);
/// assert!(!world.despawn_ent(copy));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Stores the position of the slot that the entity occupies in the world.
    index: u32,
    /// Stores the version that the slot had when the world created this entity.
    version: u32,
}

impl Entity {
    /// Returns a handle with the slot position `index` and the version `version`.
    pub(crate) fn new(index: u32, version: u32) -> Self {
        Self { index, version }
    }

    /// Returns the position of the slot that the entity occupies, as a `usize`, the type that
    /// indexes a `Vec`.
    pub(crate) fn index(self) -> usize {
        self.index as usize
    }

    /// Returns the version that the slot had when the world created this entity.
    pub(crate) fn version(self) -> u32 {
        self.version
    }
}
