use crate::Entity;
use crate::archetype::Archetype;
use crate::component::ComponentInfo;
use std::any::TypeId;
use std::collections::HashMap;

pub struct World {
    // Every table. Entry 0 is the table for entities with no components
    archetypes: Vec<Archetype>,
    // Avoids linear searches of archetype tables by hashing them against their bitmask
    archetype_indices: HashMap<u64, usize>,
    // Maps each component type to its bit, a `u64` with one set bit
    component_bits: HashMap<TypeId, u64>,
    // The version of each slot and where its entity is stored, indexed by `Entity::index`
    slots: Vec<Slot>,
    // The index of each slot that no entity occupies, which `spawn_ent` reuses
    free_slots: Vec<u32>,
}

/// [`Location`] wrapped with the entity version
#[derive(Clone, Copy, Debug)]
struct Slot {
    version: u32,       // the version of the entity that occupies the slot
    location: Location, // where that entity is stored
}

/// Where one entity is stored.
#[derive(Clone, Copy, Debug)]
struct Location {
    /// The index of the entity's table in `World::archetypes`.
    archetype: usize,
    /// The entity's row in that table.
    row: usize,
}

impl World {
    // The position in `archetypes` of the table for entities with no components
    const EMPTY: usize = 0;

    // Returns an empty world (no entities)
    pub fn new() -> Self {
        // Starts the list of tables with one table, which has no columns
        let archetypes = vec![Archetype::new(&[])];
        // Creates the map from each table's mask to the table's position in `archetypes`
        let mut archetype_indices = HashMap::new();
        // Files mask 0, which stands for no component types, under the empty table
        archetype_indices.insert(0, Self::EMPTY);

        Self {
            archetypes,                     // stores the list built above
            archetype_indices,              // stores the map built above
            component_bits: HashMap::new(), // starts with no component types, and so no bits
            slots: Vec::new(),              // starts with no entities
            free_slots: Vec::new(),         // starts with no slots to reuse
        }
    }

    pub fn spawn_ent(&mut self) -> Entity {
        // Places the new entity in the empty table, at the row just past its last entity
        let location = Location {
            archetype: Self::EMPTY,
            row: self.archetypes[Self::EMPTY].len(),
        };

        // Takes a freed slot where one exists, and adds a new slot otherwise
        let entity = match self.free_slots.pop() {
            Some(index) => {
                // Borrows the freed slot, to change it in place
                let slot = &mut self.slots[index as usize];
                // Points the slot at the new entity's row
                slot.location = location;
                // Returns a handle at the slot's version, which `despawn_ent` already raised
                Entity::new(index, slot.version)
            }
            None => {
                // Takes the next index, which equals the number of slots, and panics past u32::MAX
                let index = u32::try_from(self.slots.len())
                    .expect("the number of slots should fit in a `u32`, the type of an entity index");
                // Adds the new slot at version 0, pointing at the new entity's row
                self.slots.push(Slot {
                    version: 0,
                    location,
                });
                // Returns a handle for the new slot, at version 0
                Entity::new(index, 0)
            }
        };

        // Adds the entity to the end of the empty table, at the row that `location` records
        // SAFETY: `push_entity` requires one new component in every column, and this table has no
        // columns
        unsafe { self.archetypes[Self::EMPTY].push_entity(entity) };
        // Returns the handle to the caller
        entity
    }

    fn locate(&self, entity: Entity) -> Option<Location> {
        // Finds the entity's slot, and returns `None` where the index lies past the last slot
        let slot = self.slots.get(entity.index())?;
        // Returns `None` where the slot belongs to a later entity, since the versions then differ
        if slot.version != entity.version() {
            return None;
        }

        // Returns where the living entity is stored
        Some(slot.location)
    }

    /// Deletes `entity` and drops its components. Returns `true` where the entity existed, and `false`
    /// where it did not.
    pub fn despawn_ent(&mut self, entity: Entity) -> bool {
        // Finds where the entity is stored, and returns `false` where it does not exist
        let Some(location) = self.locate(entity) else {
            return false;
        };

        // Borrows the entity's table, to change it in place
        let archetype_table = &mut self.archetypes[location.archetype];
        // Deletes the entity's row, which moves the table's last entity into that row
        let filled_opt = archetype_table.remove_entity(location.row);
        // Records the new row of the entity that moved, where one did
        if let Some(filled) = filled_opt {
            self.slots[filled.index()].location.row = location.row;
        }

        // Finds the position of the dead entity's slot
        let index = entity.index();
        // Borrows that slot, to change it in place
        let slot = &mut self.slots[index];
        // Raises the version, which makes `locate` refuse every copy of the dead handle
        slot.version += 1;
        // Frees the slot for `spawn_ent` to reuse, unless its version has reached `u32::MAX`
        if slot.version != u32::MAX {
            self.free_slots.push(index as u32);
        }
        // Reports that the entity existed
        true
    }

    /// Inserts the component `T` into the [`Entity`]. An entity that already contains a component of
    /// type `T` has its current type overridden. An entity without a component of type `T` will
    /// move tables to the correct archetype, or create such an archetype table if one does not already
    /// exist
    pub fn insert<T: Send + Sync + 'static>(&mut self, entity: Entity, component: T) {
        // Finds where the entity is stored, and panics where it does not exist
        let location = self
            .locate(entity)
            .expect("the entity should be alive in this world");
        // Finds the identity of `T`, which a table looks its columns up by
        let type_id = TypeId::of::<T>();

        // Replaces the old `T` in place, where the entity's table already has a `T` column
        if let Some(components) = self.archetypes[location.archetype].mut_components_of(type_id) {
            // Overwrites the entity's `T`, which drops the old one
            components.as_mut_slice::<T>()[location.row] = component;
            // Returns, since the entity stays in its table
            return;
        }

        // Finds or creates the table for the entity's types plus `T`
        let target_index = self.find_or_create_archetype_with::<T>(location.archetype);
        // Lists the two tables to borrow: the entity's current table first, then the target
        let indices = [location.archetype, target_index];
        // Borrows both tables at once, after checking that the two positions differ
        let [source_table, target_table] = self
            .archetypes
            .get_disjoint_mut(indices)
            .expect("the target archetype should differ from the source, since the entity lacks a `T`");

        // Records the entity's row in the target, which is the target's current number of rows
        let row = target_table.len();
        // Moves the entity and every component that it has to the end of the target
        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the target.
        // `locate` returned this row for a living entity, and the line below pushes onto `T`, which is
        // the only new column
        let moved_ent = unsafe { source_table.move_entity(location.row, target_table) };
        // Finds the target's `T` column, which the source table lacked
        let target_column = target_table
            .mut_components_of(type_id)
            .expect("the target archetype should store `T`, since it was found or created with `T`");
        // Pushes the new component, which fills the entity's row in that column
        target_column.push_typed(component);

        // Describes where the entity now lives
        let new_location = Location {
            archetype: target_index,
            row,
        };
        // Updates the slots of the moved entity, and of the entity that filled its old row
        self.record_move(entity, moved_ent, location, new_location);
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> bool {
        // Finds where the entity is stored, and returns `false` where it does not exist
        let Some(location) = self.locate(entity) else {
            return false;
        };
        // Finds the identity of `T`, which a table uses to look up its columns
        let type_id = TypeId::of::<T>();
        // Returns `false` where the entity's table has no `T` column, since there is nothing to drop
        if self.archetypes[location.archetype].components_of(type_id).is_none() {
            return false;
        }

        // Finds or creates the table for the entity's types minus `T`
        let target = self.find_or_create_archetype_without(location.archetype, type_id);
        // Lists the two tables to borrow: the entity's current table first, then the target
        let indices = [location.archetype, target];
        // Borrows both tables at once, after checking that the two positions differ
        let [source, target_archetype] = self
            .archetypes
            .get_disjoint_mut(indices)
            .expect("the source and target archetypes are different");

        // Records the entity's row in the target, which is the target's current number of rows
        let row = target_archetype.len();
        // Moves the entity's other components to the target, and drops its `T`
        // SAFETY: `move_entity_dropping` requires a row in range, and a target whose columns all appear
        // in this table. `locate` returned this row for a living entity, and the target's columns are
        // this table's columns minus `T`
        let filled = unsafe { source.move_entity_dropping(location.row, target_archetype) };

        // Describes where the entity now lives
        let new = Location {
            archetype: target,
            row,
        };
        self.record_move(entity, filled, location, new);
        true
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components = self.archetypes[location.archetype].components_of(TypeId::of::<T>())?;
        // Returns the entity's `T`, from the entity's row in that column
        components.as_slice::<T>().get(location.row)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components = self.archetypes[location.archetype].mut_components_of(TypeId::of::<T>())?;
        // Returns the entity's `T` for changing, from the entity's row in that column
        components.as_mut_slice::<T>().get_mut(location.row)
    }

    // private helpers:

    fn find_or_create_archetype_with<T: Send + Sync + 'static>(&mut self, source: usize) -> usize {
        // Finds the bit for `T`, giving `T` a new bit where this is the first time the world sees it
        let bit = self.find_or_assign_bit(TypeId::of::<T>());
        // Builds the target table's mask: every type in the source table, plus `T`
        let mask = self.archetypes[source].mask() | bit;
        // Returns the table that already has that mask, where one exists
        if let Some(&index) = self.archetype_indices.get(&mask) {
            return index;
        }

        // Copies the descriptions of the source table's columns
        let mut infos = self.archetypes[source].infos();
        // Adds a description for `T`, with its bit
        infos.push(ComponentInfo::of::<T>(bit));

        // Takes the next position in `archetypes`, where the new table goes
        let index = self.archetypes.len();
        // Creates the new table, with one empty column for each description
        self.archetypes.push(Archetype::new(&infos));
        // Files the new table under its mask, for the next lookup to find
        self.archetype_indices.insert(mask, index);
        // Returns the new table's position
        index
    }

    fn find_or_create_archetype_without(&mut self, source: usize, type_id: TypeId) -> usize {
        // Copies the descriptions of the source table's columns
        let mut infos = self.archetypes[source].infos();
        // Throws away the description for `type_id`, keeping every other one
        infos.retain(|info| info.type_id() != type_id);

        // Builds the target table's mask, by setting the bit of each type that is left
        let mask = infos.iter().fold(0, |mask, info| {
            mask | info.bit()
        });
        // Returns the table that already has that mask, where one exists
        if let Some(&index) = self.archetype_indices.get(&mask) {
            return index;
        }

        // Takes the next position in `archetypes`, where the new table goes
        let index = self.archetypes.len();
        // Creates the new table, with one empty column for each description that is left
        self.archetypes.push(Archetype::new(&infos));
        // Files the new table under its mask, for the next lookup to find
        self.archetype_indices.insert(mask, index);
        // Returns the new table's position
        index
    }

    fn find_or_assign_bit(&mut self, type_id: TypeId) -> u64 {
        // Counts the bits handed out so far, which is also the position of the next free bit
        let next_position = self.component_bits.len();
        // Returns the type's bit, first giving it the next free bit where it has none yet
        *self.component_bits.entry(type_id).or_insert_with(|| {
            // Panics with the limit, since a 65th bit would overflow the `u64`
            assert!(next_position < 64, "a world has at most 64 component types");
            // Builds a `u64` with one set bit, at the next free position
            1 << next_position
        })
    }

    /// Records a move of `entity` from `old` to `new`, along with the entity that filled its old row.
    fn record_move(
        &mut self,
        moved_ent: Entity,
        swapped_ent: Option<Entity>,
        old_location: Location,
        new_location: Location,
    ) {
        // Points the slot of the entity that filled the old row at that row, where one did
        if let Some(swapped) = swapped_ent {
            self.slots[swapped.index()].location.row = old_location.row;
        }

        // Points the moved entity's slot at its new table and row
        self.slots[moved_ent.index()].location = new_location;
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new() // builds the same empty world as `new`
    }
}
