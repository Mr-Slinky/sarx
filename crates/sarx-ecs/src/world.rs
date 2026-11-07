//! Defines the world, which stores every entity and every component in one game.
//!
//! An entity is one thing in the game, such as a villager or a tree. A component is one piece of
//! data that an entity has, such as its position. An entity has no data apart from its components.
//!
//! # Tables
//!
//! The world stores its entities in tables, as ADR 0011 sets out. Each table is an [`Archetype`].
//! A table stores every entity that has one exact set of component types, with one column for each
//! type. Every entity with a `Position` and a `Health`, for example, lives in the same table.
//!
//! A new entity has no components. It therefore starts in the empty table, which has no columns.
//! The empty table always takes position [`World::EMPTY`] in the list of tables.
//!
//! Adding or removing a component changes the set of types that an entity has. The world then
//! moves the entity to the table for its new set. Where no such table exists yet, the world creates
//! it. A table stays in the world from then on, even once its last entity has left.
//!
//! # Masks
//!
//! The world gives each component type a bit, the first time that it meets the type. The first type
//! takes bit 0, the second takes bit 1, and the order continues up to the last bit. A mask is a
//! [`ComponentMask`] that combines the bits of a set of types. A world has room for as many
//! component types as a mask has bits, which [`ComponentMask::CAPACITY`] defines.
//!
//! Each table has a mask, which combines the bits of its types. The world keeps a map from each mask to its table.
//! It finds the table for a set of types by building the mask of that set, then looking the mask up
//! in the map.
//!
//! # Slots and versions
//!
//! An [`Entity`] is a handle, which stores an index and a version. The index selects one slot in the
//! world. A slot records where the entity is stored, as a table and a row in that table.
//!
//! The world reuses the slot of a despawned entity for a later entity. Before then, it raises the
//! version in that slot by one. A handle to the despawned entity then keeps its old version, which
//! differs from the version in the slot. The world compares the two versions on every call, and
//! treats a handle with a different version as one that does not refer to any entity. As a result,
//! an old handle never reaches the later entity that reuses its slot.
//!
//! A version stops at `u32::MAX`. A slot that reaches `u32::MAX` stays unused for the rest of the
//! world's life. A further raise would wrap the version round to 0. An old handle at version 0
//! might still exist, and that handle would then reach the later entity in the slot.
//!
//! # Rows after a move
//!
//! A table keeps its rows packed, with no gaps between them. When an entity leaves a row, the last
//! entity in the table moves into that row. Each move therefore changes the location of up to two
//! entities. One is the entity that left. The other is the entity that filled its row, unless the
//! entity left the last row. The world updates the slot of each.
//!
//! Every method that moves an entity MUST update both slots. Where a method leaves a slot pointing
//! at the wrong row, a later call would read or change the components of a different entity. A row
//! past the end of a column would instead make the later call panic, or end the program.

use crate::archetype::Archetype;
use crate::component::{ComponentInfo, ComponentVec};
use crate::mask::ComponentMask;
use crate::{Bundle, Entity};
use std::any::TypeId;
use std::collections::HashMap;

/// Stores every entity and every component in one game.
///
/// A caller creates an entity with [`spawn_empty_ent`](Self::spawn_empty_ent), which returns an
/// [`Entity`] handle. The caller then passes that handle to the other methods, to give the entity
/// components, to read and change them, to remove them, and finally to despawn the entity.
///
/// # Examples
///
/// The example creates a player with a position and health, takes 10 health from it, then despawns
/// it.
///
/// ```
/// use sarx_ecs::World;
///
/// struct Position(f32, f32);
/// struct Health(u32);
///
/// // Creates an empty world, and a player with no components
/// let mut world = World::new();
/// let player = world.spawn_empty_ent();
///
/// // Gives the player a position and health in one call
/// world.add_components(player, (Position(0.0, 0.0), Health(100)));
///
/// // Takes 10 health from the player, then reads the health back
/// if let Some(health) = world.get_mut::<Health>(player) {
///     health.0 -= 10;
/// }
/// let health = world.get::<Health>(player).map(|health| health.0);
/// assert_eq!(health, Some(90));
///
/// // Removes the player's position, then despawns the player
/// world.remove::<Position>(player);
/// world.despawn_ent(player);
///
/// // Checks that the old handle now reaches no entity
/// assert!(world.get::<Health>(player).is_none());
/// ```
pub struct World {
    /// Stores every table. Entry [`World::EMPTY`] is the table for entities with no components.
    archetypes: Vec<Archetype>,
    /// Maps the mask of each table to its position in `archetypes`. A lookup in the map finds a
    /// table without a search through every table.
    indices: HashMap<ComponentMask, usize>,
    /// Maps each component type to its bit, which is a mask with one set bit.
    component_bits: HashMap<TypeId, ComponentMask>,
    /// Stores one slot for each entity index, at that index.
    slots: Vec<Slot>,
    /// Stores the index of each slot that `spawn_empty_ent` can reuse. A slot joins the list when
    /// its entity is despawned, where its version is still below `u32::MAX`. The last index pushed
    /// is the first one reused.
    free_slots: Vec<u32>,
}

/// Records the version of one entity index, and where the entity with that version is stored.
#[derive(Clone, Copy, Debug)]
struct Slot {
    /// Stores the version of the entity that occupies the slot. In a free slot, it stores the
    /// version that the next entity in the slot will take.
    version: u32,
    /// Records where the entity is stored. In a free slot, the value is out of date until
    /// `spawn_empty_ent` overwrites it.
    location: Location,
}

/// Records the table and the row where one entity is stored.
#[derive(Clone, Copy, Debug)]
struct Location {
    /// Stores the position in `World::archetypes` of the table that contains the entity.
    arch_index: usize,
    /// Stores the row that the entity occupies in that table.
    row: usize,
}

impl World {
    /// Defines the position in `archetypes` of the table for entities with no components.
    const EMPTY: usize = 0;

    /// Returns a world with no entities and no component types.
    pub fn new() -> Self {
        // Starts the list of tables with one table, which has no columns
        let archetypes = vec![Archetype::new(&[])];
        // Creates the map from each table's mask to the table's position in `archetypes`
        let mut archetype_indices = HashMap::new();
        // Files the empty mask, which stands for no component types, under the empty table
        archetype_indices.insert(ComponentMask::EMPTY, Self::EMPTY);

        Self {
            archetypes,                     // stores the list built above
            indices: archetype_indices,     // stores the map built above
            component_bits: HashMap::new(), // starts with no component types, and so no bits
            slots: Vec::new(),              // starts with no entities
            free_slots: Vec::new(),         // starts with no slots to reuse
        }
    }

    /// Creates an entity with no components, and returns its handle.
    ///
    /// The handle reaches the entity until [`despawn_ent`](Self::despawn_ent) removes it. From then
    /// on, every method treats the handle as one that does not refer to any entity, even once a
    /// later entity takes the same index.
    ///
    /// # Panics
    ///
    /// Panics where every one of the 2^32 entity indices is taken, with none free for reuse.
    pub fn spawn_empty_ent(&mut self) -> Entity {
        // Places the new entity in the empty table, at the row just past its last entity
        let location = Location {
            arch_index: Self::EMPTY,
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
                let index = u32::try_from(self.slots.len()).expect(
                    "the number of slots should fit in a `u32`, the type of an entity index",
                );
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
        // SAFETY: `push_entity` requires one new component in every column. This table has no
        // columns, meaning there is nothing to push
        unsafe { self.archetypes[Self::EMPTY].push_entity(entity) };
        // Returns the handle to the caller
        entity
    }

    /// Removes `entity` from the world, and drops its components.
    ///
    /// Returns `true` where `entity` was alive. Returns `false` where the handle does not refer to a
    /// living entity, such as one already despawned. The world is then left unchanged.
    ///
    /// # Aborts
    ///
    /// The program aborts where a component of the entity panics in its `Drop` implementation.
    /// The panic message prints first. The panic would otherwise leave the columns in the entity's
    /// table with different lengths.
    pub fn despawn_ent(&mut self, entity: Entity) -> bool {
        // Finds where the entity is stored, and returns `false` where it does not exist
        let Some(location) = self.locate(entity) else {
            return false;
        };

        // Borrows the entity's table, to change it in place
        let archetype_table = &mut self.archetypes[location.arch_index];
        // Deletes the entity's row, which moves the table's last entity into that row
        let filled_opt = archetype_table.remove_entity(location.row);
        // Records the new row of the entity that moved, where one did
        if let Some(filled) = filled_opt {
            self.slots[filled.index()].location.row = location.row;
        }

        // Finds the index of the slot that the dead entity occupied
        let index = entity.index();
        // Borrows that slot, to change it in place
        let slot = &mut self.slots[index];
        // Raises the version, which makes `locate` refuse every copy of the dead handle
        slot.version += 1;
        // Frees the slot for `spawn_empty_ent` to reuse, unless its version has reached `u32::MAX`
        if slot.version != u32::MAX {
            self.free_slots.push(index as u32);
        }
        // Reports that the entity existed
        true
    }

    /// Gives `ent` every component in the bundle `components`, in one call.
    ///
    /// For each component in the bundle, the entity either gains it or has its existing component
    /// of that type replaced. The world drops each replaced component. The entity keeps every
    /// component that the bundle does not replace. The entity moves to another table at most once,
    /// whatever the number of components in the bundle.
    ///
    /// Each component type in the bundle takes the bound that [`add_component`](Self::add_component)
    /// places on `T`, for the same reasons.
    ///
    /// # Panics
    ///
    /// Panics where `ent` does not refer to a living entity. Also panics where the bundle lists the
    /// same component type twice. Also panics where the bundle brings in more component types than
    /// the world has room for, which [`ComponentMask::CAPACITY`] defines. Also panics where the
    /// `Drop` implementation of a replaced component panics.
    ///
    /// # Examples
    ///
    /// The example gives a player a position and a velocity in one call.
    ///
    /// ```
    /// use sarx_ecs::World;
    ///
    /// struct Position(f32, f32);
    /// struct Velocity(f32, f32);
    ///
    /// let mut world = World::new();
    /// let player = world.spawn_empty_ent();
    ///
    /// // Gives the player both components, which moves it to the table for `{Position, Velocity}`
    /// world.add_components(player, (Position(0.0, 0.0), Velocity(1.0, 0.0)));
    ///
    /// // Reads the velocity back
    /// let speed = world.get::<Velocity>(player).map(|velocity| velocity.0);
    /// assert_eq!(speed, Some(1.0));
    /// ```
    pub fn add_components<B: Bundle>(&mut self, ent: Entity, components: B) {
        // Finds where the entity is stored, and panics where it does not exist
        let location = self
            .locate(ent)
            .expect("the entity should be alive in this world");

        // Describes each type in the bundle, giving a bit to each type that the world has not met
        let infos = B::infos(self);
        // Finds or creates the table for the entity's types plus the bundle's types
        let target_index = self.find_or_create_archetype(location.arch_index, &infos);

        // Replaces the entity's components in place, where it already has every type in the bundle
        if target_index == location.arch_index {
            // Replaces the component at the entity's row in the column for each bundle type
            components.push_into(&mut self.archetypes[target_index], location.row);
            // Returns, since the entity stays in its table
            return;
        }

        // Records the entity's row in the target, which is the target's current number of rows
        let row = self.archetypes[target_index].len();
        // Moves the entity and every component that it has to the end of the target
        // SAFETY: `move_entity` requires the entity's current location, and a push onto each new
        // column in the target. `locate` returned this location. The target's new columns are those
        // for the bundle types that the entity lacked, and the line below pushes onto each of them.
        // It replaces the component in each column for a bundle type that the entity already had,
        // which leaves that column's length unchanged
        let target_table = unsafe { self.move_entity(ent, location, target_index) };
        // Pushes the bundle's new components, and replaces the ones that the entity already had
        components.push_into(target_table, row);
    }

    /// Gives `entity` the component `component`, of type `T`.
    ///
    /// Where `entity` already has a `T`, `component` replaces it, and the world drops the old `T`.
    /// Otherwise the entity gains a `T`, and keeps every component that it already has.
    ///
    /// The bound `T: 'static` means that any reference inside a `T` stays valid for the whole
    /// program. The world can then keep a `T` for as long as it needs. The bound `T: Send + Sync` lets the world
    /// move its components between threads, and share them.
    ///
    /// # Panics
    ///
    /// Panics where `entity` does not refer to a living entity. Also panics where the world has not met `T`
    /// before, and already has [`ComponentMask::CAPACITY`] component types. Also panics where the `Drop`
    /// implementation of a replaced `T` panics.
    pub fn add_component<T: Send + Sync + 'static>(&mut self, entity: Entity, component: T) {
        // Finds where the entity is stored, and panics where it does not exist
        let location = self
            .locate(entity)
            .expect("the entity should be alive in this world");
        // Finds the identity of `T`, which a table uses to look up its columns
        let type_id = TypeId::of::<T>();

        // Replaces the old `T` in place, where the entity's table already has a `T` column
        if let Some(components) = self.archetypes[location.arch_index].mut_components_of(type_id) {
            // Overwrites the entity's `T`, which drops the old one
            components.as_mut_slice::<T>()[location.row] = component;
            // Returns, since the entity stays in its table
            return;
        }

        // Finds or creates the table for the entity's types plus `T`
        let target_index = self.find_or_create_archetype_with::<T>(location.arch_index);
        // Moves the entity and every component that it has to the end of the target
        // SAFETY: `move_entity` requires the entity's current location, and a push onto each new
        // column in the target. `locate` returned this location, and the lines below push onto the
        // `T` column, which is the only new column
        let target_table = unsafe { self.move_entity(entity, location, target_index) };
        // Finds the target's `T` column, which the source table lacked
        let target_column = target_table.mut_components_of(type_id).expect(
            "the target archetype should store `T`, since it was found or created with `T`",
        );
        // Pushes the new component, which fills the entity's row in that column
        target_column.push_typed(component);
    }

    /// Removes the component of type `T` from `entity`, and drops it. The entity keeps every other
    /// component.
    ///
    /// Returns `true` where the entity had a `T`. Returns `false` where it had no `T`, or where the
    /// handle does not refer to a living entity. The world is then left unchanged.
    ///
    /// # Aborts
    ///
    /// The program aborts where the `Drop` implementation of the `T` panics. The panic message prints
    /// first. The panic would otherwise leave columns of different lengths in the entity's old and
    /// new tables.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> bool {
        // Finds where the entity is stored, and returns `false` where it does not exist
        let Some(location) = self.locate(entity) else {
            return false;
        };
        // Finds the identity of `T`, which a table uses to look up its columns
        let type_id = TypeId::of::<T>();
        // Returns `false` where the entity's table has no `T` column, since there is nothing to drop
        if self.archetypes[location.arch_index]
            .components_of(type_id)
            .is_none()
        {
            return false;
        }

        // Finds or creates the table for the entity's types minus `T`
        let target = self.find_or_create_archetype_without(location.arch_index, type_id);
        // Moves the entity's other components to the target, and drops its `T`
        // SAFETY: `move_entity` requires the entity's current location, and a push onto each new
        // column in the target. `locate` returned this location, and the target's columns are the
        // source table's columns minus `T`, meaning the target has no new columns
        unsafe { self.move_entity(entity, location, target) };
        // Reports that the entity had a `T`
        true
    }

    /// Returns the component of type `T` that `entity` has. Returns `None` where the entity has no
    /// `T`, or where the handle does not refer to a living entity.
    ///
    /// The returned reference borrows the world, meaning the compiler rejects any change to the world
    /// while the reference is in use.
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components = self.archetypes[location.arch_index].components_of(TypeId::of::<T>())?;
        // Returns the entity's `T`, from the entity's row in that column
        components.as_slice::<T>().get(location.row)
    }

    /// Returns the component of type `T` that `entity` has, for the caller to change. Returns `None`
    /// where the entity has no `T`, or where the handle does not refer to a living entity.
    ///
    /// The returned reference borrows the world exclusively, meaning the compiler rejects any other
    /// use of the world while the reference is in use.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components = self.archetypes[location.arch_index].mut_components_of(TypeId::of::<T>())?;
        // Returns the entity's `T` for changing, from the entity's row in that column
        components.as_mut_slice::<T>().get_mut(location.row)
    }

    /// Returns the bit for the type that `type_id` identifies. Gives the type the next free bit where
    /// it has none yet.
    ///
    /// # Panics
    ///
    /// Panics where the type has no bit yet, and the world already has [`ComponentMask::CAPACITY`]
    /// component types.
    pub(crate) fn find_or_assign_bit(&mut self, type_id: TypeId) -> ComponentMask {
        // Counts the bits handed out so far, which is also the position of the next free bit
        let next_position = self.component_bits.len();
        // Returns the type's bit, first giving it the next free bit where it has none yet
        *self.component_bits.entry(type_id).or_insert_with(|| {
            // Panics where every bit is taken, since a mask has no bit left for another type
            let capacity = ComponentMask::CAPACITY;
            assert!(
                next_position < capacity,
                "a world has at most {capacity} component types"
            );
            // Builds a mask with one set bit, at the next free position
            ComponentMask::single(next_position)
        })
    }

    // ========================================================================================== \\
    //                                       Helper Methods                                       \\
    // ========================================================================================== \\

    /// Returns where `entity` is stored, or `None` where the handle does not refer to a living
    /// entity.
    ///
    /// A handle does not refer to a living entity where its index lies past the last slot. It also does
    /// not refer to one where its version differs from the version in its slot, as it does for a
    /// despawned entity.
    fn locate(&self, entity: Entity) -> Option<Location> {
        // Finds the entity's slot, and returns `None` where the index lies past the last slot
        let slot = self.slots.get(entity.index())?;
        // Returns `None` where the versions differ, as they do once the entity is despawned
        if slot.version != entity.version() {
            return None;
        }

        // Returns where the living entity is stored
        Some(slot.location)
    }

    /// Returns the position in `archetypes` of the table for the types in table `source`, plus the
    /// types that `bundle_infos` describes. Creates that table where the world has none yet.
    ///
    /// A bundle type that table `source` already stores does not add a column. The entity keeps that
    /// column, and the bundle replaces its component there.
    ///
    /// # Panics
    ///
    /// Panics where `bundle_infos` describes the same type twice.
    fn find_or_create_archetype(&mut self, source: usize, bundle_infos: &[ComponentInfo]) -> usize {
        // Combines the bit of every bundle type into one mask
        let mut bundle_mask = ComponentMask::EMPTY;
        for info in bundle_infos {
            // Stops a bundle that lists a type twice, which would give the new table two columns
            // for one type
            assert!(
                !bundle_mask.intersects(info.bit()),
                "a bundle lists the same component type twice"
            );
            bundle_mask |= info.bit();
        }

        // Finds the types that the entity already has
        let source_mask = self.archetypes[source].mask();
        // Builds the target table's mask, which combines the entity's types with the bundle's types
        let mask = source_mask | bundle_mask;
        // Returns the table that already has that mask, if one exists
        if let Some(&index) = self.indices.get(&mask) {
            return index;
        }

        // Copies the descriptions of the source table's columns
        let mut infos = self.archetypes[source].infos();
        // Selects the description of each bundle type that the entity lacks. Each other bundle type
        // replaces the component in a column that the source table already has
        let dedup_bundle_infos = bundle_infos
            .iter()
            .filter(|info| !source_mask.intersects(info.bit()));
        // Adds those descriptions, one for each new column in the target table
        infos.extend(dedup_bundle_infos);

        // Creates the new table, and returns its position
        self.create_archetype(&infos)
    }

    /// Returns the position in `archetypes` of the table for the types in table `source`, plus `T`.
    /// Creates that table where the world has none yet.
    ///
    /// # Panics
    ///
    /// Panics where the world has not met `T` before, and already has [`ComponentMask::CAPACITY`]
    /// component types.
    fn find_or_create_archetype_with<T: Send + Sync + 'static>(&mut self, source: usize) -> usize {
        // Finds the bit for `T`, giving `T` a new bit where this is the first time the world sees it
        let bit = self.find_or_assign_bit(TypeId::of::<T>());
        // Builds the target table's mask, which combines every type in the source table with `T`
        let mask = self.archetypes[source].mask() | bit;
        // Returns the table that already has that mask, where one exists
        if let Some(&index) = self.indices.get(&mask) {
            return index;
        }

        // Copies the descriptions of the source table's columns
        let mut infos = self.archetypes[source].infos();
        // Adds a description for `T`, with its bit
        infos.push(ComponentInfo::of::<T>(bit));

        // Creates the new table, and returns its position
        self.create_archetype(&infos)
    }

    /// Returns the position in `archetypes` of the table for the types in table `source`, minus the
    /// type that `type_id` identifies. Creates that table where the world has none yet.
    fn find_or_create_archetype_without(&mut self, source: usize, type_id: TypeId) -> usize {
        // Copies the descriptions of the source table's columns
        let mut infos = self.archetypes[source].infos();
        // Throws away the description for `type_id`, keeping every other one
        infos.retain(|info| info.type_id() != type_id);

        // Builds the target table's mask, by setting the bit of each type that is left
        let mask = infos
            .iter()
            .fold(ComponentMask::EMPTY, |mask, info| mask | info.bit());
        // Returns the table that already has that mask, where one exists
        if let Some(&index) = self.indices.get(&mask) {
            return index;
        }

        // Creates the new table, and returns its position
        self.create_archetype(&infos)
    }

    /// Creates a table with one empty column for each description in `infos`, and returns its
    /// position in `archetypes`.
    ///
    /// The method also files the table in `indices`, under the mask that the table builds from
    /// `infos`. The mask and the columns therefore always describe the same types.
    ///
    /// The caller MUST first check that `indices` has no table under that mask. A second table
    /// would replace the first in `indices`. A lookup would then find only the second table, and
    /// entities with that set of types would end up split across two tables.
    ///
    /// # Panics
    ///
    /// Panics in a debug build where two entries in `infos` share a bit.
    fn create_archetype(&mut self, infos: &[ComponentInfo]) -> usize {
        // Takes the next position in `archetypes`, where the new table goes
        let index = self.archetypes.len();
        // Creates the new table, which builds its mask from `infos`
        let archetype = Archetype::new(infos);
        // Files the new table under its own mask, for the next lookup to find
        self.indices.insert(archetype.mask(), index);
        // Adds the new table at that position
        self.archetypes.push(archetype);
        // Returns the new table's position
        index
    }

    /// Moves `entity` from its table to the end of the table at position `target`, and returns
    /// the target table for the caller to fill.
    ///
    /// The method drops each component that the target has no column for. It also updates the slot
    /// of the moved entity, and the slot of the entity that fills its old row.
    ///
    /// The returned reference borrows the world exclusively, meaning the compiler rejects any other
    /// use of the world until the caller has finished filling the target.
    ///
    /// # Panics
    ///
    /// Panics where `target` equals `location.archetype`, since `get_disjoint_mut` refuses to borrow
    /// one table as both the source and the target.
    ///
    /// # Aborts
    ///
    /// The program aborts where the `Drop` implementation of a dropped component panics, as
    /// [`Archetype::move_entity_dropping`] sets out.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee two things:
    ///
    /// - `location` is where `entity` is stored now, as [`locate`](Self::locate) returns it. A stale
    ///   row could lie past the end of a column.
    /// - The caller pushes one component onto each column that the target has and the source table
    ///   lacks, before anything else uses the target. A missing component leaves that column one
    ///   row short, as [`Archetype::push_entity`] sets out.
    unsafe fn move_entity(
        &mut self,
        entity: Entity,
        location: Location,
        target: usize,
    ) -> &mut Archetype {
        // Lists the two tables to borrow, with the entity's current table first and the target second
        let indices = [location.arch_index, target];
        // Borrows both tables at once, after checking that the two positions differ
        let [source_table, target_table] = self
            .archetypes
            .get_disjoint_mut(indices)
            .expect("the target archetype should differ from the entity's own table");

        // Records the entity's row in the target, which is the target's current number of rows
        let row = target_table.len();
        // Moves the entity to the end of the target, and finds the entity that fills its old row,
        // where one does
        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in
        // the target. The caller MUST guarantee both, as the Safety section of this method sets out
        let moved_ent = unsafe { source_table.move_entity_dropping(location.row, target_table) };

        // Describes where the entity now lives
        let new_location = Location {
            arch_index: target,
            row,
        };
        // Updates the slot of `entity`, and the slot of `moved_ent`, the entity that filled its old row
        self.record_move(entity, moved_ent, location, new_location);
        // Returns the target, for the caller to push onto its new columns
        &mut self.archetypes[target]
    }

    /// Updates the slots after `moved_ent` moves from `old_location` to `new_location`.
    ///
    /// `swapped_ent` is the entity that filled the row that `moved_ent` left, or `None` where that row
    /// was the last one in its table.
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
    /// Returns a world with no entities and no component types, as [`World::new`] does.
    fn default() -> Self {
        Self::new() // builds the same empty world as `new`
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use std::array;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, PartialEq)]
    struct Position(f32);

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    /// Adds 1 to a shared counter when dropped, which shows how many times clean-up code ran.
    struct DropCounter {
        drops: Arc<AtomicUsize>,
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Builds an array with the `TypeId` of each array type `[u8; N]`, for every `N` listed.
    macro_rules! type_ids {
        ($($length:literal)*) => { [$(TypeId::of::<[u8; $length]>()),*] };
    }

    /// Returns 64 distinct `TypeId`s, enough to take every bit that a world can hand out.
    fn build_64_type_ids() -> [TypeId; 64] {
        type_ids!(
             0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
            16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
            32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
            48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63
        )
    }

    /// Returns where `entity` is stored, and panics where it is not alive.
    fn locate_alive(world: &World, entity: Entity) -> Location {
        world.locate(entity).expect("the entity should be alive")
    }

    /// Returns a counter that adds to `drops` when dropped.
    fn build_counter(drops: &Arc<AtomicUsize>) -> DropCounter {
        DropCounter {
            drops: Arc::clone(drops),
        }
    }

    /// Checks that a new world has one table with no columns, and no entities or component types
    #[test]
    fn new_with_no_args_returns_world_with_only_empty_table() {
        let world = World::new();

        let empty_table = &world.archetypes[World::EMPTY];
        assert_eq!(world.archetypes.len(), 1, "number of tables");
        assert_eq!(
            empty_table.mask(),
            ComponentMask::EMPTY,
            "mask of the empty table"
        );
        assert_eq!(empty_table.len(), 0, "rows in the empty table");
        let empty_index = world.indices.get(&ComponentMask::EMPTY);
        assert_eq!(
            empty_index,
            Some(&World::EMPTY),
            "table under the empty mask"
        );
        assert!(world.component_bits.is_empty(), "component types");
        assert!(world.slots.is_empty(), "slots");
        assert!(world.free_slots.is_empty(), "free slots");
    }

    /// Checks that `default` builds the same empty world as `new`
    #[test]
    fn default_with_no_args_returns_empty_world() {
        let world = World::default();

        assert_eq!(world.archetypes.len(), 1, "number of tables");
        assert!(world.slots.is_empty(), "slots");
    }

    /// Checks that a world with no freed slots hands out indices in order, each at version 0
    #[test]
    fn spawn_ent_with_varying_counts_returns_sequential_handles_at_version_zero() {
        let cases = [1_u32, 2, 5];

        for count in cases {
            let mut world = World::new();

            let entities: Vec<Entity> = (0..count).map(|_| world.spawn_empty_ent()).collect();

            let expected: Vec<Entity> = (0..count).map(|index| Entity::new(index, 0)).collect();
            let rows = world.archetypes[World::EMPTY].len();
            assert_eq!(entities, expected, "handles for {count} spawns");
            assert_eq!(rows, count as usize, "rows for {count} spawns");
        }
    }

    /// Checks that each spawned entity lives in the empty table, at the row of its spawn order
    #[test]
    fn spawn_ent_with_several_spawns_places_each_in_next_row_of_empty_table() {
        let mut world = World::new();

        let entities = [
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
        ];

        for (row, entity) in entities.into_iter().enumerate() {
            let location = locate_alive(&world, entity);
            assert_eq!(location.arch_index, World::EMPTY, "table of entity {row}");
            assert_eq!(location.row, row, "row of entity {row}");
        }
    }

    /// Checks that a freed slot is reused, at a version one above the dead entity's
    #[test]
    fn spawn_ent_with_freed_slot_reuses_index_at_raised_version() {
        let mut world = World::new();
        let dead = world.spawn_empty_ent();
        world.despawn_ent(dead);

        let reborn = world.spawn_empty_ent();

        assert_eq!(reborn, Entity::new(0, 1), "handle of the new entity");
        assert_eq!(world.slots.len(), 1, "slots");
        assert!(world.free_slots.is_empty(), "free slots");
    }

    /// Checks that freed slots come back last freed first, since `free_slots` is a stack
    #[test]
    fn spawn_ent_with_several_freed_slots_reuses_last_freed_first() {
        let mut world = World::new();
        let first = world.spawn_empty_ent();
        let second = world.spawn_empty_ent();
        world.spawn_empty_ent();
        world.despawn_ent(first);
        world.despawn_ent(second);

        let reborn = [
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
        ];

        assert_eq!(
            reborn,
            [Entity::new(1, 1), Entity::new(0, 1), Entity::new(3, 0)]
        );
    }

    /// Checks that a despawned entity is gone, and that the call reports that it existed
    #[test]
    fn despawn_ent_with_living_entity_returns_true_and_forgets_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Health(10));

        let despawned = world.despawn_ent(entity);

        assert!(despawned, "answer for a living entity");
        assert!(world.locate(entity).is_none(), "location after despawning");
        assert_eq!(
            world.get::<Health>(entity),
            None,
            "component after despawning"
        );
        assert_eq!(world.free_slots, [0], "free slots");
    }

    /// Checks that a handle to a despawned entity, to an index never spawned, or at the wrong version
    /// gives `false`
    #[test]
    fn despawn_ent_with_handle_of_no_living_entity_returns_false() {
        let cases = [
            ("despawned", Entity::new(0, 0)),
            ("index past the last slot", Entity::new(5, 0)),
            ("later version", Entity::new(0, 7)),
            ("largest index and version", Entity::new(u32::MAX, u32::MAX)),
        ];

        for (label, handle) in cases {
            let mut world = World::new();
            let entity = world.spawn_empty_ent();
            world.despawn_ent(entity);

            let despawned = world.despawn_ent(handle);

            assert!(!despawned, "answer for {label}");
            assert_eq!(world.free_slots, [0], "free slots for {label}");
        }
    }

    /// Checks that the entity that fills the empty row can still be found, with its own component
    #[test]
    fn despawn_ent_with_first_row_updates_location_of_filling_entity() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world.add_component(entity, Health(value as u32));
        }
        let [first, second, third] = entities;

        world.despawn_ent(first);

        assert_eq!(
            locate_alive(&world, third).row,
            0,
            "row of the filling entity"
        );
        assert_eq!(
            world.get::<Health>(second),
            Some(&Health(1)),
            "second entity's component"
        );
        assert_eq!(
            world.get::<Health>(third),
            Some(&Health(2)),
            "third entity's component"
        );
    }

    /// Checks that despawning drops each of the entity's components once, and no others
    #[test]
    fn despawn_ent_with_drop_counter_drops_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let doomed = world.spawn_empty_ent();
        let survivor = world.spawn_empty_ent();
        world.add_component(doomed, build_counter(&drops));
        world.add_component(survivor, build_counter(&drops));

        world.despawn_ent(doomed);

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that a slot whose version reaches `u32::MAX` is never reused, since its next version
    /// would wrap round to one that an old handle may still have
    #[test]
    fn despawn_ent_with_version_reaching_max_retires_slot() {
        let mut world = World::new();
        world.spawn_empty_ent();
        world.slots[0].version = u32::MAX - 1;
        let last_version = Entity::new(0, u32::MAX - 1);

        let despawned = world.despawn_ent(last_version);
        let next = world.spawn_empty_ent();

        assert!(despawned, "answer for the entity at the last version");
        assert_eq!(
            world.slots[0].version,
            u32::MAX,
            "version of the retired slot"
        );
        assert_eq!(next, Entity::new(1, 0), "handle of the next entity");
    }

    /// Checks that a first component moves the entity out of the empty table, into a new table
    #[test]
    fn insert_with_new_component_moves_entity_to_new_table() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();

        world.add_component(entity, Position(1.5));

        let location = locate_alive(&world, entity);
        let bit = world.component_bits[&TypeId::of::<Position>()];
        let mask = world.archetypes[location.arch_index].mask();
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(location.arch_index, 1, "table of the entity");
        assert_eq!(mask, bit, "mask of the new table");
        assert_eq!(
            world.archetypes[World::EMPTY].len(),
            0,
            "rows in the empty table"
        );
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.5)),
            "component"
        );
    }

    /// Checks that a second component keeps the first, and moves the entity to a table with both
    #[test]
    fn insert_with_second_component_keeps_first_component() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Position(1.5));

        world.add_component(entity, Health(100));

        let location = locate_alive(&world, entity);
        let mask = world.archetypes[location.arch_index].mask();
        assert_eq!(world.archetypes.len(), 3, "number of tables");
        assert_eq!(
            mask,
            ComponentMask::from_positions(&[0, 1]),
            "mask of the entity's table"
        );
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.5)),
            "first component"
        );
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(100)),
            "second component"
        );
    }

    /// Checks that a component of a type the entity already has replaces the old one in place
    #[test]
    fn insert_with_existing_component_type_replaces_value_in_place() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Health(1));
        let before = locate_alive(&world, entity);

        world.add_component(entity, Health(2));

        let after = locate_alive(&world, entity);
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(after.arch_index, before.arch_index, "table of the entity");
        assert_eq!(after.row, before.row, "row of the entity");
        assert_eq!(world.get::<Health>(entity), Some(&Health(2)), "component");
    }

    /// Checks that replacing a component drops the old one, and leaves the new one alive
    #[test]
    fn insert_with_existing_component_type_drops_old_value_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, build_counter(&drops));

        world.add_component(entity, build_counter(&drops));

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that entities with the same component types share one table
    #[test]
    fn insert_with_same_types_on_two_entities_reuses_table() {
        let mut world = World::new();
        let first = world.spawn_empty_ent();
        let second = world.spawn_empty_ent();
        world.add_component(first, Health(1));

        world.add_component(second, Health(2));

        let first_location = locate_alive(&world, first);
        let second_location = locate_alive(&world, second);
        let rows = [first_location.row, second_location.row];
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            first_location.arch_index, second_location.arch_index,
            "shared table"
        );
        assert_eq!(rows, [0, 1], "rows");
    }

    /// Checks that an entity ends in the same table, whichever order its components arrive in
    #[test]
    fn insert_with_types_in_either_order_ends_in_same_table() {
        let mut world = World::new();
        let first = world.spawn_empty_ent();
        let second = world.spawn_empty_ent();
        world.add_component(first, Position(1.0));
        world.add_component(first, Health(1));

        world.add_component(second, Health(2));
        world.add_component(second, Position(2.0));

        let first_table = locate_alive(&world, first).arch_index;
        let second_table = locate_alive(&world, second).arch_index;
        let masks = world.indices.len();
        assert_eq!(first_table, second_table, "table of each entity");
        assert_eq!(masks, world.archetypes.len(), "one mask for each table");
    }

    /// Checks that moving one entity leaves every other entity with its own components
    #[test]
    fn insert_with_entity_leaving_middle_row_keeps_other_components_in_place() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
            world.spawn_empty_ent(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world.add_component(entity, Health(value as u32));
        }
        let [first, second, third] = entities;

        world.add_component(second, Position(9.0));

        assert_eq!(
            locate_alive(&world, third).row,
            1,
            "row of the filling entity"
        );
        assert_eq!(
            world.get::<Health>(first),
            Some(&Health(0)),
            "first entity's component"
        );
        assert_eq!(
            world.get::<Health>(second),
            Some(&Health(1)),
            "second entity's component"
        );
        assert_eq!(
            world.get::<Health>(third),
            Some(&Health(2)),
            "third entity's component"
        );
        assert_eq!(
            world.get::<Position>(second),
            Some(&Position(9.0)),
            "new component"
        );
    }

    /// Checks that moving an entity to a new table does not drop any of its components
    #[test]
    fn insert_with_drop_counter_moving_tables_drops_nothing() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, build_counter(&drops));

        world.add_component(entity, Health(1));

        assert_eq!(drops.load(Ordering::Relaxed), 0);
    }

    /// Checks that a component type that takes no memory still moves the entity to its own table
    #[test]
    fn insert_with_zero_size_type_moves_entity_to_new_table() {
        struct Tag;

        let mut world = World::new();
        let entity = world.spawn_empty_ent();

        world.add_component(entity, Tag);

        assert_eq!(
            locate_alive(&world, entity).arch_index,
            1,
            "table of the entity"
        );
        assert!(world.get::<Tag>(entity).is_some(), "component");
    }

    /// Checks that `add_component` panics for an entity that is not alive, since the entity has no
    /// row to fill
    #[test]
    #[should_panic]
    fn insert_with_despawned_entity_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.despawn_ent(entity);

        world.add_component(entity, Health(1));
    }

    /// Checks that `add_component` panics on a 65th component type, since a mask has room for 64 bits
    #[test]
    #[should_panic]
    fn insert_with_65th_component_type_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        for type_id in build_64_type_ids() {
            world.find_or_assign_bit(type_id);
        }

        world.add_component(entity, Health(1));
    }

    /// Checks that 64 component types take every bit of the mask, one bit each
    #[test]
    fn find_or_assign_bit_with_64_types_assigns_every_bit_once() {
        let mut world = World::new();

        let bits = build_64_type_ids().map(|type_id| world.find_or_assign_bit(type_id));

        let expected: [ComponentMask; 64] = array::from_fn(ComponentMask::single);
        assert_eq!(bits, expected, "one bit each, in order of first sight");
    }

    /// Checks that a type seen before keeps its first bit, and does not take a new one
    #[test]
    fn find_or_assign_bit_with_repeated_type_returns_same_bit() {
        let mut world = World::new();
        let first = world.find_or_assign_bit(TypeId::of::<Position>());
        let other = world.find_or_assign_bit(TypeId::of::<Health>());

        let repeat = world.find_or_assign_bit(TypeId::of::<Position>());

        assert_eq!(repeat, first, "bit for the repeated type");
        let expected = [ComponentMask::single(0), ComponentMask::single(1)];
        assert_eq!([first, other], expected, "bits in order of first sight");
        assert_eq!(world.component_bits.len(), 2, "component types");
    }

    /// Checks that removing a component drops it, and keeps the entity's other components
    #[test]
    fn remove_with_stored_component_returns_true_and_keeps_other_components() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Position(1.5));
        world.add_component(entity, Health(100));

        let removed = world.remove::<Health>(entity);

        let location = locate_alive(&world, entity);
        let position_bit = world.component_bits[&TypeId::of::<Position>()];
        let mask = world.archetypes[location.arch_index].mask();
        assert!(removed, "answer for a stored component");
        assert_eq!(world.get::<Health>(entity), None, "removed component");
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.5)),
            "kept component"
        );
        assert_eq!(mask, position_bit, "mask of the entity's table");
    }

    /// Checks that removing the only component returns the entity to the empty table
    #[test]
    fn remove_with_only_component_returns_entity_to_empty_table() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Health(1));

        world.remove::<Health>(entity);

        let location = locate_alive(&world, entity);
        assert_eq!(location.arch_index, World::EMPTY, "table of the entity");
        assert_eq!(world.archetypes.len(), 2, "number of tables");
    }

    /// Checks that removal finds the table that `add_component` built, rather than building a second one
    #[test]
    fn remove_with_existing_target_table_reuses_table() {
        let mut world = World::new();
        let reference = world.spawn_empty_ent();
        let entity = world.spawn_empty_ent();
        world.add_component(reference, Position(1.0));
        world.add_component(entity, Position(2.0));
        world.add_component(entity, Health(2));

        world.remove::<Health>(entity);

        let reference_table = locate_alive(&world, reference).arch_index;
        let entity_table = locate_alive(&world, entity).arch_index;
        assert_eq!(entity_table, reference_table, "table of each entity");
        assert_eq!(world.archetypes.len(), 3, "number of tables");
    }

    /// Checks that removal reaches a table that `add_component` never built, and builds it
    #[test]
    fn remove_with_new_target_table_creates_table() {
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, Position(1.0));
        world.add_component(entity, Health(1));

        world.remove::<Position>(entity);

        let location = locate_alive(&world, entity);
        let health_bit = world.component_bits[&TypeId::of::<Health>()];
        let mask = world.archetypes[location.arch_index].mask();
        assert_eq!(world.archetypes.len(), 4, "number of tables");
        assert_eq!(mask, health_bit, "mask of the new table");
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(1)),
            "kept component"
        );
    }

    /// Checks that `remove` returns `false` for a missing type, and for an entity that is not alive
    #[test]
    fn remove_with_nothing_to_remove_returns_false() {
        let mut world = World::new();
        let bare = world.spawn_empty_ent();
        let with_position = world.spawn_empty_ent();
        let despawned = world.spawn_empty_ent();
        world.add_component(with_position, Position(1.0));
        world.add_component(despawned, Health(1));
        world.despawn_ent(despawned);

        let answers = [
            ("entity with no components", world.remove::<Health>(bare)),
            (
                "entity with another type",
                world.remove::<Health>(with_position),
            ),
            ("despawned entity", world.remove::<Health>(despawned)),
        ];

        let kept = world.get::<Position>(with_position);
        for (label, removed) in answers {
            assert!(!removed, "answer for {label}");
        }
        assert_eq!(kept, Some(&Position(1.0)), "untouched component");
    }

    /// Checks that removing a component drops it once, and does not drop anything else
    #[test]
    fn remove_with_drop_counter_drops_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty_ent();
        world.add_component(entity, build_counter(&drops));
        world.add_component(entity, Health(1));

        world.remove::<DropCounter>(entity);

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that moving one entity out leaves the entity that fills its row with its own components
    #[test]
    fn remove_with_entity_leaving_first_row_keeps_other_components_in_place() {
        let mut world = World::new();
        let first = world.spawn_empty_ent();
        let second = world.spawn_empty_ent();
        for (value, entity) in [first, second].into_iter().enumerate() {
            world.add_component(entity, Position(value as f32));
            world.add_component(entity, Health(value as u32));
        }

        world.remove::<Health>(first);

        assert_eq!(
            locate_alive(&world, second).row,
            0,
            "row of the filling entity"
        );
        assert_eq!(
            world.get::<Position>(second),
            Some(&Position(1.0)),
            "filling entity's position"
        );
        assert_eq!(
            world.get::<Health>(second),
            Some(&Health(1)),
            "filling entity's health"
        );
        assert_eq!(
            world.get::<Position>(first),
            Some(&Position(0.0)),
            "moved entity's position"
        );
    }

    /// Checks `get` for a stored component, a missing type, and entities that are not alive
    #[test]
    fn get_with_varying_entities_returns_component_only_where_stored() {
        let mut world = World::new();
        let stored = world.spawn_empty_ent();
        let bare = world.spawn_empty_ent();
        let despawned = world.spawn_empty_ent();
        world.add_component(stored, Health(5));
        world.add_component(despawned, Health(6));
        world.despawn_ent(despawned);

        let cases = [
            ("stored component", stored, Some(Health(5))),
            ("missing type", bare, None),
            ("despawned entity", despawned, None),
            ("never spawned", Entity::new(99, 0), None),
        ];

        for (label, entity, expected) in cases {
            let found = world.get::<Health>(entity);
            assert_eq!(found, expected.as_ref(), "component for {label}");
        }
    }

    /// Checks that a handle kept past despawning never reaches the new entity in the same slot
    #[test]
    fn get_with_stale_handle_to_reused_slot_returns_none() {
        let mut world = World::new();
        let stale = world.spawn_empty_ent();
        world.add_component(stale, Health(1));
        world.despawn_ent(stale);
        let reborn = world.spawn_empty_ent();
        world.add_component(reborn, Health(2));

        let through_stale = world.get::<Health>(stale);
        let through_reborn = world.get::<Health>(reborn);

        assert_eq!(through_stale, None, "component through the stale handle");
        assert_eq!(
            through_reborn,
            Some(&Health(2)),
            "component through the new handle"
        );
    }

    /// Checks that a change through `get_mut` stays in the world, for the one entity changed
    #[test]
    fn get_mut_with_stored_component_keeps_change() {
        let mut world = World::new();
        let changed = world.spawn_empty_ent();
        let untouched = world.spawn_empty_ent();
        world.add_component(changed, Health(1));
        world.add_component(untouched, Health(1));

        if let Some(health) = world.get_mut::<Health>(changed) {
            health.0 = 50;
        }

        assert_eq!(
            world.get::<Health>(changed),
            Some(&Health(50)),
            "changed entity"
        );
        assert_eq!(
            world.get::<Health>(untouched),
            Some(&Health(1)),
            "other entity"
        );
    }

    /// Checks that `get_mut` returns `None` for a missing type, and for an entity that is not alive
    #[test]
    fn get_mut_with_nothing_stored_returns_none() {
        let mut world = World::new();
        let bare = world.spawn_empty_ent();
        let despawned = world.spawn_empty_ent();
        world.add_component(despawned, Health(1));
        world.despawn_ent(despawned);

        let missing_type = world.get_mut::<Health>(bare).is_none();
        let not_alive = world.get_mut::<Health>(despawned).is_none();

        assert!(missing_type, "answer for a missing type");
        assert!(not_alive, "answer for a despawned entity");
    }

    /// Checks that dropping the world drops every component that it still stores, once each
    #[test]
    fn drop_with_components_in_several_tables_drops_each_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let alone = world.spawn_empty_ent();
        let paired = world.spawn_empty_ent();
        world.add_component(alone, build_counter(&drops));
        world.add_component(paired, build_counter(&drops));
        world.add_component(paired, Health(1));

        drop(world);

        assert_eq!(drops.load(Ordering::Relaxed), 2);
    }
}
