//! Defines the handle that refers to one entity in a world, and the allocator that hands handles
//! out.
//!
//! The world documentation sets out how a world uses the index and the version in each handle.
//! [`Entities`] stores one slot for each index, and keeps the versions up to date.
//!
//! The example on [`Entity`] spawns an entity, copies its handle, and despawns the entity.

use crate::world::Location;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Returns the position of the slot that the entity occupies, as the `u32` that the handle
    /// stores.
    ///
    /// The free list of `Entities` stores indices as `u32`. This method hands the index over as
    /// it is, where [`index`](Self::index) would widen it to a `usize` that the caller then
    /// narrows back.
    ///
    /// The example on [`Entity::new`] creates a handle for slot 5, and this method returns `5`.
    pub(crate) fn raw_index(self) -> u32 {
        self.index
    }

    /// Returns the version that the slot had when the world created this entity.
    ///
    /// The example on [`Entity::new`] creates a handle at version 2, and this method returns `2`.
    pub(crate) fn version(self) -> u32 {
        self.version
    }
}

/// Hands out entity handles, and records where each living entity is stored.
///
/// The allocator stores one slot for each entity index. A slot records the version of its index,
/// and the location of the entity that uses the index. A freed entity's index goes on a free
/// list, which [`alloc`](Self::alloc) takes from before it adds a new slot.
///
/// # Examples
///
/// The example allocates two entities, frees the first, and allocates again. The third entity
/// reuses index 0, at version 1.
///
/// ```ignore
/// let mut entities = Entities::new();
/// let first = entities.alloc(Location::new(0, 0));
/// let second = entities.alloc(Location::new(0, 1));
///
/// // Frees index 0, which raises its version to 1
/// entities.free(first);
///
/// // Reuses index 0, and refuses the old handle
/// let third = entities.alloc(Location::new(0, 0));
/// assert_eq!(third, Entity::new(0, 1));
/// assert!(entities.locate(first).is_none());
/// ```
pub(crate) struct Entities {
    /// Stores one slot for each entity index, at that index.
    ///
    /// For example, entry 3 stores the slot for every handle with index 3.
    slots: Vec<Slot>,
    /// Stores the index of each slot that [`alloc`](Self::alloc) can reuse, with the last freed on
    /// top.
    ///
    /// For example, once a caller frees the entity at index 2, the list contains 2.
    free_slots: Vec<u32>,
}

impl Entities {
    /// Returns an allocator with no slots, and nothing to reuse.
    ///
    /// The example on [`Entities`] shows a call.
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    /// Returns a handle for a new entity stored at `location`.
    ///
    /// The method reuses the slot freed most recently, at the version that [`free`](Self::free)
    /// raised it to. Where no slot is free, it adds a slot at version 0.
    ///
    /// # Panics
    ///
    /// Panics where every one of the 2^32 entity indices is taken, with none free for reuse.
    ///
    /// # Examples
    ///
    /// In a new allocator, the first call returns index 0 at version 0. The example on
    /// [`Entities`] shows a call that reuses a freed slot.
    pub(crate) fn alloc(&mut self, location: Location) -> Entity {
        match self.free_slots.pop() {
            Some(index) => {
                let slot = &mut self.slots[index as usize];
                slot.location = location;
                // Returns a handle at the slot's version, which `free` already raised
                Entity::new(index, slot.version)
            }
            None => {
                // Takes the next index, which equals the number of slots, and panics past `u32::MAX`
                let index = u32::try_from(self.slots.len()).expect(
                    "the number of slots should fit in a `u32`, the type of an entity index",
                );
                self.slots.push(Slot {
                    version: 0,
                    location,
                });
                Entity::new(index, 0)
            }
        }
    }

    /// Ends the life of `entity`, after which [`locate`](Self::locate) refuses every copy of its
    /// handle.
    ///
    /// The method raises the version in the entity's slot, and puts the slot on the free list. A
    /// slot whose version reaches `u32::MAX` is retired instead, and never reused. The world
    /// documentation sets out why.
    ///
    /// The caller MUST pass a living entity. A debug build panics where it does not. A release
    /// build would raise the version of whichever entity occupies the slot, and end its life.
    ///
    /// # Examples
    ///
    /// The example on [`Entities`] frees an entity at index 0, which raises its version to 1.
    pub(crate) fn free(&mut self, entity: Entity) {
        let slot = &mut self.slots[entity.index()];
        debug_assert_eq!(
            slot.version,
            entity.version(),
            "only a living entity can be freed"
        );
        slot.version += 1;
        // Frees the slot for `alloc` to reuse, unless its version has reached `u32::MAX`
        if slot.version != u32::MAX {
            self.free_slots.push(entity.raw_index());
        }
    }

    /// Returns where `entity` is stored, or `None` where the handle does not refer to a living
    /// entity.
    ///
    /// A handle does not refer to a living entity where its index lies past the last slot. It also
    /// does not refer to one where its version differs from the version in its slot, as it does for
    /// a freed entity.
    ///
    /// # Examples
    ///
    /// The example on [`Entities`] locates a freed handle, which returns `None`.
    pub(crate) fn locate(&self, entity: Entity) -> Option<Location> {
        let slot = self.slots.get(entity.index())?;
        if slot.version != entity.version() {
            return None;
        }
        Some(slot.location)
    }

    /// Records that `entity` is now stored at `location`.
    ///
    /// The caller MUST pass a living entity. For example, once the world moves an entity to row 3
    /// of table 1, it calls `set_location(entity, Location::new(1, 3))`.
    pub(crate) fn set_location(&mut self, entity: Entity, location: Location) {
        self.slots[entity.index()].location = location;
    }

    /// Records that `entity` now occupies row `row`, in the table that it was already in.
    ///
    /// The caller MUST pass a living entity. For example, once the entity in the last row of a
    /// table fills row 0, the world calls `set_row(entity, 0)`.
    ///
    /// # Panics
    ///
    /// Panics where `row` exceeds `u32::MAX`.
    pub(crate) fn set_row(&mut self, entity: Entity, row: usize) {
        self.slots[entity.index()].location.set_row(row);
    }

    /// Makes room for `additional` more entities, beyond those that the free list will take.
    ///
    /// For example, with two slots on the free list, `reserve(5)` makes room for three new slots.
    ///
    /// # Panics
    ///
    /// Panics where the new capacity exceeds `isize::MAX` bytes.
    pub(crate) fn reserve(&mut self, additional: usize) {
        let new_slots = additional.saturating_sub(self.free_slots.len());
        self.slots.reserve(new_slots);
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Records the version of one entity index, and where the entity with that version is stored.
///
/// For example, a slot with version 1 and the location `Location::new(0, 4)` belongs to the second
/// entity to use its index. That entity occupies row 4 of the empty table.
#[derive(Clone, Copy, Debug)]
struct Slot {
    /// Stores the version of the entity that occupies the slot. In a free slot, it stores the
    /// version that the next entity in the slot will take.
    ///
    /// For example, once a caller frees the first entity in a slot, the slot stores version 1.
    version: u32,
    /// Records where the entity is stored. In a free slot, the value is out of date until `alloc`
    /// overwrites it.
    location: Location,
}

// Stops the build where the size of a slot differs from 12 bytes. A slot needs 4 bytes for its
// version, and 4 bytes for each of its two location fields
const _: () = assert!(size_of::<Slot>() == 12);

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
impl Entities {
    /// Returns the number of slots, whether live, free or retired.
    pub(crate) fn count_slots(&self) -> usize {
        self.slots.len()
    }

    /// Returns how many slots the list has room for before it next grows.
    pub(crate) fn slot_capacity(&self) -> usize {
        self.slots.capacity()
    }

    /// Returns the freed indices, with the next one that `alloc` reuses last.
    pub(crate) fn read_free_list(&self) -> &[u32] {
        &self.free_slots
    }

    /// Returns the version and the location in the slot at `index`.
    pub(crate) fn read_slot(&self, index: usize) -> (u32, Location) {
        let slot = self.slots[index];
        (slot.version, slot.location)
    }

    /// Overwrites the version in the slot at `index`.
    pub(crate) fn set_version(&mut self, index: usize, version: u32) {
        self.slots[index].version = version;
    }
}

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

    /// Returns the table and the row that `location` records.
    fn read_place(location: Location) -> (usize, usize) {
        (location.arch_index(), location.row())
    }

    /// Returns an allocator with `count` living entities, where entity `n` occupies row `n` of
    /// table 0, along with their handles.
    fn build_entities(count: usize) -> (Entities, Vec<Entity>) {
        let mut entities = Entities::new();
        let handles = (0..count)
            .map(|row| entities.alloc(Location::new(0, row)))
            .collect();
        (entities, handles)
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

    /// Checks that an allocator with no freed slots hands out indices in order, each at version 0
    #[test]
    fn alloc_with_no_freed_slots_returns_sequential_handles_at_version_zero() {
        let cases = [1_u32, 2, 5];

        let found = cases.map(|count| {
            let expected: Vec<Entity> = (0..count).map(|index| Entity::new(index, 0)).collect();
            let mut entities = Entities::new();
            let handles: Vec<Entity> = (0..count)
                .map(|row| entities.alloc(Location::new(0, row as usize)))
                .collect();
            (count, handles, expected, entities.count_slots())
        });

        for (count, handles, expected, slots) in found {
            assert_eq!(handles, expected, "handles for {count} allocations");
            assert_eq!(slots, count as usize, "slots for {count} allocations");
        }
    }

    /// Checks that freed slots come back last freed first, each at its raised version, before any
    /// new slot is added
    #[test]
    fn alloc_with_freed_slots_reuses_last_freed_first_at_raised_version() {
        let (mut entities, handles) = build_entities(3);
        entities.free(handles[0]);
        entities.free(handles[1]);

        let reused = [
            entities.alloc(Location::new(1, 0)),
            entities.alloc(Location::new(1, 1)),
            entities.alloc(Location::new(1, 2)),
        ];

        assert_eq!(
            reused,
            [Entity::new(1, 1), Entity::new(0, 1), Entity::new(3, 0)],
            "handles"
        );
        assert!(entities.read_free_list().is_empty(), "free slots");
        assert_eq!(entities.count_slots(), 4, "slots");
    }

    /// Checks that a new handle records the location that `alloc` received, in a new slot and in a
    /// reused one
    #[test]
    fn alloc_with_location_records_location_for_locate() {
        let (mut entities, handles) = build_entities(1);
        entities.free(handles[0]);

        let reused = entities.alloc(Location::new(2, 7));
        let added = entities.alloc(Location::new(3, 9));

        let reused_place = entities.locate(reused).map(read_place);
        let added_place = entities.locate(added).map(read_place);
        assert_eq!(reused_place, Some((2, 7)), "location of the reused slot");
        assert_eq!(added_place, Some((3, 9)), "location of the new slot");
    }

    /// Checks that freeing an entity raises the version in its slot and puts the slot on the free
    /// list, after which `locate` refuses the old handle
    #[test]
    fn free_with_living_entity_raises_version_and_frees_slot() {
        let (mut entities, handles) = build_entities(3);

        entities.free(handles[1]);

        assert_eq!(entities.read_slot(1).0, 1, "version of the freed slot");
        assert_eq!(entities.read_free_list(), [1], "free slots");
        assert!(entities.locate(handles[1]).is_none(), "freed handle");
        assert!(entities.locate(handles[0]).is_some(), "first handle");
        assert!(entities.locate(handles[2]).is_some(), "third handle");
    }

    /// Checks that a slot whose version reaches `u32::MAX` is retired rather than freed, since its
    /// next version would wrap round to one that an old handle may still have
    #[test]
    fn free_with_version_reaching_max_retires_slot() {
        let (mut entities, _) = build_entities(1);
        entities.set_version(0, u32::MAX - 1);
        let last_version = Entity::new(0, u32::MAX - 1);

        entities.free(last_version);
        let next = entities.alloc(Location::new(0, 0));

        assert_eq!(
            entities.read_slot(0).0,
            u32::MAX,
            "version of the retired slot"
        );
        assert!(entities.read_free_list().is_empty(), "free slots");
        assert_eq!(next, Entity::new(1, 0), "handle of the next entity");
    }

    /// Checks that a debug build catches a second free of the same handle, which would otherwise
    /// end the life of whichever entity holds the slot next
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn free_with_freed_handle_panics() {
        let (mut entities, handles) = build_entities(1);
        entities.free(handles[0]);

        entities.free(handles[0]);
    }

    /// Checks that `locate` finds a living entity, and refuses a freed handle, an index past the
    /// last slot and a version that differs from the slot's
    #[test]
    fn locate_with_living_and_dead_handles_returns_location_only_for_living() {
        let (mut entities, handles) = build_entities(3);
        entities.free(handles[2]);

        // Pairs each handle with the table and row that `locate` should find
        let cases = [
            ("first living entity", handles[0], Some((0, 0))),
            ("second living entity", handles[1], Some((0, 1))),
            ("freed entity", handles[2], None),
            ("index past the last slot", Entity::new(3, 0), None),
            ("later version", Entity::new(0, 1), None),
            (
                "largest index and version",
                Entity::new(u32::MAX, u32::MAX),
                None,
            ),
        ];

        let found = cases.map(|(label, handle, expected)| {
            let place = entities.locate(handle).map(read_place);
            (label, place, expected)
        });

        for (label, place, expected) in found {
            assert_eq!(place, expected, "location for {label}");
        }
    }

    /// Checks that `set_location` moves one entity to a new table and row, and leaves the others
    #[test]
    fn set_location_with_living_entity_changes_only_its_location() {
        let (mut entities, handles) = build_entities(2);

        entities.set_location(handles[0], Location::new(4, 2));

        let moved = entities.locate(handles[0]).map(read_place);
        let other = entities.locate(handles[1]).map(read_place);
        assert_eq!(moved, Some((4, 2)), "location of the moved entity");
        assert_eq!(other, Some((0, 1)), "location of the other entity");
    }

    /// Checks that `set_row` changes the row of one entity, and keeps its table
    #[test]
    fn set_row_with_living_entity_keeps_table() {
        let (mut entities, handles) = build_entities(2);
        entities.set_location(handles[1], Location::new(3, 8));

        entities.set_row(handles[1], 0);

        let place = entities.locate(handles[1]).map(read_place);
        assert_eq!(place, Some((3, 0)));
    }

    /// Checks that `set_row` panics on a row past `u32::MAX`
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic]
    fn set_row_with_row_past_u32_max_panics() {
        let (mut entities, handles) = build_entities(1);

        entities.set_row(handles[0], u32::MAX as usize + 1);
    }

    /// Checks that `reserve` makes room for every entity that the free list cannot take
    #[test]
    fn reserve_with_no_freed_slots_makes_room_for_every_entity() {
        let cases = [0, 1, 10];

        let found = cases.map(|additional| {
            let (mut entities, _) = build_entities(2);
            entities.reserve(additional);
            (additional, entities.slot_capacity())
        });

        for (additional, capacity) in found {
            assert!(
                capacity >= 2 + additional,
                "capacity {capacity} after reserving {additional}"
            );
        }
    }

    /// Checks that `reserve` adds room only for the entities that the freed slots cannot take
    ///
    /// The list of slots starts full, with two of its slots freed. A reserve of 102 then needs
    /// room for 100 new slots. The request is far above double the capacity, meaning the list grows
    /// to exactly what it needs, and room for all 102 would show in the capacity
    #[test]
    fn reserve_with_partial_free_list_makes_room_beyond_freed_slots() {
        let mut entities = Entities::new();
        let mut handles = vec![entities.alloc(Location::new(0, 0))];
        while entities.count_slots() < entities.slot_capacity() {
            let row = entities.count_slots();
            handles.push(entities.alloc(Location::new(0, row)));
        }
        entities.free(handles[0]);
        entities.free(handles[1]);
        let slots = entities.count_slots();

        entities.reserve(102);

        let capacity = entities.slot_capacity();
        assert!(
            (slots + 100..slots + 102).contains(&capacity),
            "capacity {capacity} for {slots} slots"
        );
    }

    /// Checks that `reserve` leaves the list of slots at its capacity, where the freed slots can
    /// take every entity that it makes room for
    #[test]
    fn reserve_with_enough_freed_slots_keeps_slot_capacity() {
        let mut entities = Entities::new();
        let mut handles = vec![entities.alloc(Location::new(0, 0))];
        while entities.count_slots() < entities.slot_capacity() {
            let row = entities.count_slots();
            handles.push(entities.alloc(Location::new(0, row)));
        }
        for &handle in &handles {
            entities.free(handle);
        }
        let capacity = entities.slot_capacity();

        entities.reserve(handles.len());

        assert_eq!(
            entities.slot_capacity(),
            capacity,
            "room in the list of slots"
        );
    }
}
