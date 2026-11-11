//! Defines the world, which stores every entity and every component in one game.
//!
//! An entity is one thing in the game, such as a villager or a tree. A component is one piece of
//! data that an entity has, such as its position. An entity has no data apart from its components.
//!
//! The example on [`World`] follows one entity from its spawn to its despawn.
//!
//! # Tables
//!
//! The world stores its entities in tables, as ADR 0011 sets out. Each table is an [`Archetype`].
//! A table stores every entity that has one exact set of component types, with one column for each
//! type. Every entity with a `Position` and a `Health`, for example, lives in the same table. Every
//! entity with a `Position`, a `Health` and a `Velocity` lives in another table.
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
//! Each table has a mask, which combines the bits of its types. The world keeps a map from each mask
//! to its table. It finds the table for a set of types by building the mask of that set, then
//! looking the mask up in the map.
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
//! A slot serves at most `u32::MAX` entities over the life of the world. Once a despawn raises the
//! version in a slot to `u32::MAX`, the world retires the slot. The world never reuses that slot,
//! and its index stays unused for the rest of the world's life.
//!
//! The retirement protects old handles. A further raise would wrap the version round to 0. A handle
//! from the first entity in the slot, at version 0, could still exist. That handle would then
//! incorrectly reach whichever entity took the slot next.
//!
//! # Rows after a move
//!
//! A table keeps its rows packed, with no gaps between them. When an entity leaves a row, the last
//! entity in the table moves into that row. Each move therefore changes the location of up to two
//! entities. One is the entity that left. The other is the entity that filled its row, unless the
//! entity left the last row. The world updates the slot of each.
//!
//! Every method that moves an entity MUST update both slots. If a method leaves a slot pointing
//! at the wrong row, a later call would read or change the components of a different entity. A row
//! past the end of a column would instead make the later call panic, or end the program.

use crate::archetype::Archetype;
use crate::component::{Component, ComponentInfo, ComponentVec};
use crate::mask::ComponentMask;
use crate::query::Permission;
use crate::{query, Bundle, Entity, Query, ReadOnlyQuery};
use std::any::TypeId;
use std::collections::HashMap;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Stores every entity and every component in one game.
///
/// A caller creates an entity with [`spawn_empty`](Self::spawn_empty), which returns an
/// [`Entity`] handle. The caller then passes that handle to the other methods, to give the entity
/// components, to read and change them, to remove them, and finally to despawn the entity.
///
/// # Examples
///
/// The example creates a player with a position and health, and takes 10 health from it. It then
/// removes the position, which comes back as a value, and despawns the player with its health still
/// in place.
///
/// ```
/// use enki::World;
/// # use enki::Component;
///
/// #[derive(Debug)]
/// struct Position(f32, f32);
/// #[derive(Debug)]
/// struct Health(u32);
/// # impl Component for Position {}
/// # impl Component for Health {}
///
/// // Creates an empty world, and a player with no components
/// let mut world = World::new();
/// let player = world.spawn_empty();
///
/// // Gives the player a position and health in one call
/// world
///     .add_components(player, (Position(0.0, 0.0), Health(100)))
///     .expect("the player should be alive");
///
/// // Takes 10 health from the player, then reads the health back
/// if let Some(health) = world.get_mut::<Health>(player) {
///     health.0 -= 10;
/// }
/// let health = world.get::<Health>(player).map(|health| health.0);
/// assert_eq!(health, Some(90));
///
/// // Removes the player's position and reads the value that comes back, which leaves its health in
/// // place
/// let removed = world
///     .remove_component::<Position>(player)
///     .map(|position| (position.0, position.1));
/// assert_eq!(removed, Some((0.0, 0.0)));
/// assert!(world.get::<Position>(player).is_none());
///
/// // Despawns the player, which drops its health along with it
/// world.despawn(player);
///
/// // Checks that the old handle now reaches no entity
/// assert!(world.get::<Health>(player).is_none());
/// ```
pub struct World {
    /// Stores every table. Entry [`World::EMPTY`] is the table for entities with no components.
    ///
    /// The world never removes a table. Each table therefore keeps its position in this list for
    /// the life of the world, and `indices` can store that position.
    ///
    /// For example, a world whose only entity has a `Position` stores two tables. Entry 0 is the
    /// empty table, and entry 1 is the table for `{Position}`.
    archetypes: Vec<Archetype>,
    /// Maps the mask of each table to its position in `archetypes`.
    ///
    /// For example, a new world maps [`ComponentMask::EMPTY`] to [`World::EMPTY`].
    indices: HashMap<ComponentMask, usize>,
    /// Maps each component type to its bit.
    ///
    /// For example, a world that met `Position` first maps `TypeId::of::<Position>()` to the mask
    /// with bit 0 set.
    arch_masks: HashMap<TypeId, ComponentMask>,
    /// Stores one slot for each entity index, at that index.
    ///
    /// For example, entry 3 stores the slot for every handle with index 3.
    slots: Vec<Slot>,
    /// Stores the index of each slot that `spawn_empty` can reuse.
    ///
    /// For example, once a caller despawns the entity at index 2, the list contains 2.
    free_slots: Vec<u32>,
}

impl World {
    /// Defines the position in `archetypes` of the table for entities that have no components.
    ///
    /// For example, `self.archetypes[Self::EMPTY]` returns the table where
    /// [`spawn_empty`](Self::spawn_empty) places each new entity.
    const EMPTY: usize = 0;

    /// Defines how many component types one world has room for.
    ///
    /// The world gives each component type a bit, the first time that it meets the type. Once every
    /// bit is taken, a call that brings in one more type panics.
    ///
    /// # Examples
    ///
    /// The example reads the limit.
    ///
    /// ```
    /// use enki::World;
    ///
    /// // Reads the number of component types that a world has room for
    /// assert_eq!(World::MAX_COMPONENT_TYPES, 64);
    /// ```
    pub const MAX_COMPONENT_TYPES: usize = ComponentMask::CAPACITY;

    /// Returns a world with no entities and no component types.
    ///
    /// The example on [`World`] shows a call.
    #[must_use]
    pub fn new() -> Self {
        // Starts the list of tables with one table, which has no columns
        let archetypes = vec![Archetype::new(&[])];
        // Creates the map from each table's mask to the table's position in `archetypes`
        let mut archetype_indices = HashMap::new();
        // Files the empty mask, which has no bits set, under the empty table
        archetype_indices.insert(ComponentMask::EMPTY, Self::EMPTY);

        Self {
            archetypes,
            indices: archetype_indices,
            arch_masks: HashMap::new(),
            slots: Vec::new(),
            free_slots: Vec::new(),
        }
    }

    /// Creates an entity with no components, and returns its handle.
    ///
    /// The handle reaches the entity until [`despawn`](Self::despawn) removes it. From then
    /// on, every method treats the handle as one that does not refer to any entity, even once a
    /// later entity takes the same index.
    ///
    /// The example on [`World`] shows a call.
    ///
    /// # Panics
    ///
    /// Panics where every one of the 2^32 entity indices is taken, with none free for reuse.
    pub fn spawn_empty(&mut self) -> Entity {
        // Places the new entity in the empty table, at the row just past its last entity
        let row = self.archetypes[Self::EMPTY].len();
        let location = Location::new(Self::EMPTY, row);

        // Takes a freed slot where one exists, and adds a new slot otherwise
        let entity = match self.free_slots.pop() {
            Some(index) => {
                // Borrows the freed slot, to change it in place
                let slot = &mut self.slots[index as usize];
                // Points the slot at the new entity's row
                slot.location = location;
                // Returns a handle at the slot's version, which `despawn` already raised
                Entity::new(index, slot.version)
            }
            None => {
                // Takes the next index, which equals the number of slots, and panics past `u32::MAX`
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

        // Adds the entity to the end of the empty table, at the row that `location` records. The
        // table has no columns, so there is nothing to push
        self.archetypes[Self::EMPTY].push_entity(entity, |_, _| {});
        // Returns the handle to the caller
        entity
    }

    /// Removes `entity` from the world, and drops its components.
    ///
    /// Returns `true` where `entity` was alive. Returns `false` where the handle does not refer to a
    /// living entity, such as one already despawned. The world is then left unchanged.
    ///
    /// The example on [`World`] shows a call. The example on [`Entity`] shows a second call with a
    /// copy of the same handle, which returns `false`.
    ///
    /// # Aborts
    ///
    /// The program aborts where a component of the entity panics in its `Drop` implementation.
    /// The panic message prints first. The panic would otherwise leave the columns in the entity's
    /// table with different lengths.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        // Finds where the entity is stored, and returns `false` where it does not exist
        let Some(location) = self.locate(entity) else {
            return false;
        };

        // Borrows the entity's table, to change it in place
        let archetype_table = &mut self.archetypes[location.arch_index()];
        // Deletes the entity's row, which moves the table's last entity into that row
        let filled_opt = archetype_table.remove_entity(location.row());
        // Records the new row of the entity that moved, where one did
        self.record_filled_row(filled_opt, location.row());

        let index = entity.index();
        // Borrows the entity's slot, to change it in place
        let slot = &mut self.slots[index];
        // Raises the version, which makes `locate` refuse every copy of the dead handle
        slot.version += 1;
        // Frees the slot for `spawn_empty` to reuse, unless its version has reached `u32::MAX`
        if slot.version != u32::MAX {
            self.free_slots.push(index as u32);
        }
        // Reports that the entity existed
        true
    }

    /// Makes room for at least `additional` more entities that have exactly the component types in
    /// the bundle type `B`.
    ///
    /// The world finds or creates the table for the types in `B`. It then makes room in every
    /// column of that table, and in its entity list. It also makes room in the list of slots, for
    /// each of the `additional` entities that a freed slot will not take.
    ///
    /// The table and the list of slots then keep their blocks of memory while the caller spawns
    /// the next `additional` entities and gives each one a `B`. The empty table, where each new
    /// entity starts, grows as usual.
    ///
    /// The world gives a bit to each type in `B` that it has not met before. It keeps the table
    /// from then on, as it keeps every other table.
    ///
    /// # Panics
    ///
    /// Panics where `B` lists the same component type twice. Also panics where `B` brings in more
    /// component types than the world has room for, which
    /// [`MAX_COMPONENT_TYPES`](Self::MAX_COMPONENT_TYPES) defines.
    ///
    /// Also panics where the required capacity overflows a `usize`, or where a larger block would
    /// exceed `isize::MAX` bytes. The program aborts where the allocator cannot supply the block.
    ///
    /// # Examples
    ///
    /// The example makes room for 100 entities with a position and health, then creates them.
    ///
    /// ```
    /// use enki::World;
    /// # use enki::Component;
    ///
    /// #[derive(Debug)]
    /// struct Position(f32, f32);
    /// #[derive(Debug)]
    /// struct Health(u32);
    /// # impl Component for Position {}
    /// # impl Component for Health {}
    ///
    /// let mut world = World::new();
    ///
    /// // Makes room for 100 entities with a `Position` and a `Health`
    /// world.reserve::<(Position, Health)>(100);
    ///
    /// // Creates the 100 entities, which the reserved table takes without growing
    /// for _ in 0..100 {
    ///     let entity = world.spawn_empty();
    ///     world
    ///         .add_components(entity, (Position(0.0, 0.0), Health(100)))
    ///         .expect("the entity should be alive");
    /// }
    /// ```
    pub fn reserve<B: Bundle>(&mut self, additional: usize) {
        // Describes each type in the bundle, giving a bit to each type that the world has not met
        let infos = B::infos(self);
        let arch_index = self.find_or_create_archetype(Self::EMPTY, &infos);
        self.archetypes[arch_index].reserve(additional);

        let new_slots = additional.saturating_sub(self.free_slots.len());
        self.slots.reserve(new_slots);
    }

    /// Gives `entity` every component in the bundle `components`, in one call.
    ///
    /// For each component in the bundle, the entity either gains it or has its existing component
    /// of that type replaced. The world drops each replaced component. The entity keeps every
    /// component that the bundle does not replace. The entity moves to another table at most once,
    /// whatever the number of components in the bundle.
    ///
    /// Each component type in the bundle must implement [`Component`].
    ///
    /// Returns `Ok(())` where the entity now has every component in the bundle. Returns `Err` with
    /// the whole bundle where the handle does not refer to a living entity. The world is then left
    /// unchanged, and the caller owns the bundle again.
    ///
    /// # Panics
    ///
    /// Panics where the bundle lists the same component type twice. Also panics where the bundle
    /// brings in more component types than the world has room for, which
    /// [`MAX_COMPONENT_TYPES`](Self::MAX_COMPONENT_TYPES) defines. Also panics where the `Drop`
    /// implementation of a replaced component panics. The world drops the replaced components last,
    /// meaning every entity is already where the world records it, and a caller that catches the
    /// panic can go on using the world.
    ///
    /// The [crate example](crate#examples) gives a villager three components with one call.
    pub fn add_components<B: Bundle>(&mut self, entity: Entity, components: B) -> Result<(), B> {
        // Finds where the entity is stored, and hands the bundle back where it does not exist
        let Some(location) = self.locate(entity) else {
            return Err(components);
        };

        // Describes each type in the bundle, giving a bit to each type that the world has not met
        let infos = B::infos(self);
        // Finds or creates the table for the entity's types plus the bundle's types
        let target_index = self.find_or_create_archetype(location.arch_index(), &infos);

        // Records the types that the entity has now, which `push_into` replaces rather than pushes
        let existing = self.archetypes[location.arch_index()].mask();

        // Replaces the entity's components in place, where it already has every type in the bundle
        if target_index == location.arch_index() {
            let table = &mut self.archetypes[target_index];
            let replaced = components.push_into(table, location.row(), existing);
            drop(replaced);
            return Ok(());
        }

        // Moves the entity to the end of the target, pushing the bundle's new components and
        // replacing the ones that the entity already had. The closure hands the replaced
        // components out, since dropping them inside the move would run their `Drop` before the
        // slots record the move
        let mut replaced = None;
        // The target has every column that the source table has, meaning `take` never runs
        self.move_entity(
            entity,
            location,
            target_index,
            ComponentVec::swap_remove,
            |table, row| {
                replaced = Some(components.push_into(table, row, existing));
            },
        );
        drop(replaced);
        Ok(())
    }

    /// Gives `entity` the component `component`, of type `T`.
    ///
    /// Where `entity` already has a `T`, `component` replaces it, and the world drops the old `T`.
    /// Otherwise the entity gains a `T`, and keeps every component that it already has.
    ///
    /// `T` must implement [`Component`], which sets out the bounds that every component type meets.
    ///
    /// Returns `Ok(())` where the entity now has `component`. Returns `Err(component)` where the
    /// handle does not refer to a living entity. The world is then left unchanged, and the caller
    /// owns `component` again.
    ///
    /// The example on [`Component`] shows a call.
    ///
    /// # Panics
    ///
    /// Panics where the world has not met `T` before, and already has
    /// [`MAX_COMPONENT_TYPES`](Self::MAX_COMPONENT_TYPES) component types. Also panics where the
    /// `Drop` implementation of a replaced `T` panics.
    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) -> Result<(), T> {
        // Adds the component as a bundle of one, which replaces an existing `T` in place, then
        // unwraps the bundle that comes back for a dead entity
        self.add_components(entity, (component,))
            .map_err(|(component,)| component)
    }

    /// Removes the component of type `T` from `entity`, and returns it. The entity keeps every other
    /// component.
    ///
    /// `T` must implement [`Component`], which sets out the bounds that every component type meets.
    ///
    /// Returns `Some` with the removed `T` where the entity had one. Returns `None` where it had no
    /// `T`, or where the handle does not refer to a living entity. The world is then left unchanged.
    ///
    /// The caller owns the returned `T`. A call that ignores the result drops the `T` at the end of
    /// the statement, after the world has recorded the move. A panic in that `Drop` therefore leaves
    /// the world usable.
    ///
    /// The example on [`World`] removes a component and reads the value that comes back.
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the identity of `T`, which a table uses to look up its columns
        let type_id = TypeId::of::<T>();
        // Returns `None` where the entity's table has no `T` column, since there is nothing to take
        self.archetypes[location.arch_index()].components_of(type_id)?;

        // Finds or creates the table for the entity's types minus `T`
        let target = self.find_or_create_archetype_without(location.arch_index(), type_id);
        // Moves the entity's other components to the target, and takes its `T` out of the `T`
        // column. The target has no new columns, so there is nothing to push
        let mut removed = None;
        self.move_entity(
            entity,
            location,
            target,
            |components, row| removed = Some(components.swap_remove_typed::<T>(row)),
            |_, _| {},
        );
        removed
    }

    /// Returns the component of type `T` that `entity` has. Returns `None` where the entity has no
    /// `T`, or where the handle does not refer to a living entity.
    ///
    /// `T` must implement [`Component`], which sets out the bounds that every component type meets.
    ///
    /// The returned reference borrows the world, meaning the compiler rejects any change to the world
    /// while the reference is in use.
    ///
    /// The example on [`World`] shows a call.
    #[must_use]
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components = self.archetypes[location.arch_index()].components_of(TypeId::of::<T>())?;
        // Returns the entity's `T`, from the entity's row in that column
        components.as_slice::<T>().get(location.row())
    }

    /// Returns the component of type `T` that `entity` has, for the caller to change. Returns `None`
    /// where the entity has no `T`, or where the handle does not refer to a living entity.
    ///
    /// `T` must implement [`Component`], which sets out the bounds that every component type meets.
    ///
    /// The returned reference borrows the world exclusively, meaning the compiler rejects any other
    /// use of the world while the reference is in use.
    ///
    /// The example on [`World`] shows a call.
    #[must_use]
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        // Finds where the entity is stored, and returns `None` where it does not exist
        let location = self.locate(entity)?;
        // Finds the `T` column in the entity's table, and returns `None` where the table has none
        let components =
            self.archetypes[location.arch_index()].components_of_mut(TypeId::of::<T>())?;
        // Returns the entity's `T` for changing, from the entity's row in that column
        components.as_mut_slice::<T>().get_mut(location.row())
    }

    /// Returns the bit for the type that `type_id` identifies. Gives the type the next free bit where
    /// it has none yet.
    ///
    /// # Panics
    ///
    /// Panics where the type has no bit yet, and the world already has [`ComponentMask::CAPACITY`]
    /// component types.
    ///
    /// # Examples
    ///
    /// In a new world, the first type that the method meets takes bit 0, and the second takes
    /// bit 1. A later call for the first type returns bit 0 again. The test
    /// `find_or_assign_bit_with_repeated_type_returns_same_bit` checks the repeat.
    pub(crate) fn find_or_assign_bit(&mut self, type_id: TypeId) -> ComponentMask {
        // Counts the bits handed out so far, which is also the position of the next free bit
        let next_position = self.arch_masks.len();
        // Returns the type's bit, first giving it the next free bit where it has none yet
        *self.arch_masks.entry(type_id).or_insert_with(|| {
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

    /// Returns an iterator with one item for each table that stores every component type in the
    /// query `Q`. The item contains the columns that `Q` asks for from that table.
    ///
    /// Each item contains whole columns, such as a `&mut [Position]`, rather than one entity. A
    /// caller that works on a column at a time uses this method. A caller that works on one entity
    /// at a time uses [`query_mut`](Self::query_mut) instead. Every table that the iterator visits
    /// has at least one entity. The iterator is empty where `Q` asks for a type that the world has
    /// not met.
    ///
    /// The iterator borrows the world exclusively, meaning the compiler rejects any other use of
    /// the world while the iterator or a column is in use. A caller whose query only reads uses
    /// [`query_archetype`](Self::query_archetype), which borrows the world through a shared
    /// reference.
    ///
    /// # Panics
    ///
    /// Panics where `Q` changes a component type that it also asks for elsewhere, as in
    /// `(&mut Position, &Position)`.
    ///
    /// # Examples
    ///
    /// The example moves each entity that has a position and a velocity by its velocity, one table
    /// at a time.
    ///
    /// ```
    /// use enki::World;
    /// # use enki::Component;
    ///
    /// #[derive(Debug)]
    /// struct Position(f32, f32);
    /// #[derive(Debug)]
    /// struct Velocity(f32, f32);
    /// # impl Component for Position {}
    /// # impl Component for Velocity {}
    ///
    /// let mut world = World::new();
    /// let player = world.spawn_empty();
    /// world
    ///     .add_components(player, (Position(0.0, 0.0), Velocity(1.0, 2.0)))
    ///     .expect("the player should be alive");
    ///
    /// // Moves each position by its velocity, working through the columns of each table
    /// for (positions, velocities) in world.query_archetype_mut::<(&mut Position, &Velocity)>() {
    ///     for (position, velocity) in positions.iter_mut().zip(velocities) {
    ///         position.0 += velocity.0;
    ///         position.1 += velocity.1;
    ///     }
    /// }
    ///
    /// let moved = world.get::<Position>(player).map(|position| (position.0, position.1));
    /// assert_eq!(moved, Some((1.0, 2.0)));
    /// ```
    pub fn query_archetype_mut<Q: Query>(&mut self) -> impl Iterator<Item = Q::Columns<'_>> {
        // Panics where `Q` changes a component type that it also asks for elsewhere
        let permissions = query::check_permissions::<Q>();
        let query_mask = self.find_query_mask(&permissions);
        // Visits each table that has entities and stores every type in the query
        self.find_matching_archetypes(query_mask)
            // SAFETY: `find_columns` requires three guarantees. `&mut self` keeps every other
            // reference off the world for as long as the iterator lives, and `check_permissions`
            // passed above. The iterator visits each table once, meaning it builds one slice of
            // each column, and no other slice of a column that `Q` changes exists
            .filter_map(|archetype| unsafe { Q::find_columns(archetype) })
    }

    /// Returns an iterator with one item for each entity that has every component type in the
    /// query `Q`.
    ///
    /// The item for a query `&T` is a `&T`, and for `&mut T` it is a `&mut T`. For a tuple, the
    /// item is a tuple that contains the item for each field. The iterator visits the entities one
    /// table at a time, and in row order within each table. The iterator is empty where `Q` asks
    /// for a type that the world has not met.
    ///
    /// The iterator borrows the world exclusively, meaning the compiler rejects any other use of
    /// the world while the iterator or an item is in use.
    ///
    /// # Panics
    ///
    /// Panics where `Q` changes a component type that it also asks for elsewhere, as in
    /// `(&mut Position, &Position)`.
    ///
    /// # Examples
    ///
    /// The example moves every entity that has a position and a velocity by its velocity. An entity
    /// with a position alone stays where it is.
    ///
    /// ```
    /// use enki::World;
    /// # use enki::Component;
    ///
    /// #[derive(Debug)]
    /// struct Position(f32, f32);
    /// #[derive(Debug)]
    /// struct Velocity(f32, f32);
    /// # impl Component for Position {}
    /// # impl Component for Velocity {}
    ///
    /// let mut world = World::new();
    /// let player = world.spawn_empty();
    /// world
    ///     .add_components(player, (Position(0.0, 0.0), Velocity(1.0, 2.0)))
    ///     .expect("the player should be alive");
    /// let tree = world.spawn_empty();
    /// world.add_component(tree, Position(5.0, 5.0)).expect("the tree should be alive");
    ///
    /// // Moves each entity that has both components by its velocity
    /// for (position, velocity) in world.query_mut::<(&mut Position, &Velocity)>() {
    ///     position.0 += velocity.0;
    ///     position.1 += velocity.1;
    /// }
    ///
    /// let player_position = world
    ///     .get::<Position>(player)
    ///     .map(|position| (position.0, position.1));
    /// let tree_position = world.get::<Position>(tree).map(|position| (position.0, position.1));
    /// assert_eq!(player_position, Some((1.0, 2.0)));
    /// assert_eq!(tree_position, Some((5.0, 5.0)));
    /// ```
    pub fn query_mut<Q: Query>(&mut self) -> impl Iterator<Item = Q::Item<'_>> {
        self.query_archetype_mut::<Q>().flat_map(Q::split_rows)
    }

    /// Returns an iterator with one item for each table that stores every component type in the
    /// query `Q`. The item contains the columns that `Q` asks for from that table.
    ///
    /// Each item contains whole columns, such as a `&[Position]`. A caller that works on one entity
    /// at a time uses [`query`](Self::query) instead. Every table that the iterator visits has at
    /// least one entity. The iterator is empty where `Q` asks for a type that the world has not
    /// met.
    ///
    /// The compiler accepts `Q` only where it implements [`ReadOnlyQuery`], as `&Position` and
    /// `(&Position, &Velocity)` do. A caller passes a query that changes a component, such as
    /// `&mut Position`, to [`query_archetype_mut`](Self::query_archetype_mut) instead.
    ///
    /// The iterator borrows the world through a shared reference. As a result, the compiler rejects
    /// any change to the world while the iterator or a column is in use, and accepts other reads.
    ///
    /// # Examples
    ///
    /// The example adds up the first coordinate of every position, one table at a time. The world
    /// stores the player and the tree in different tables, since the player also has a velocity.
    ///
    /// ```
    /// use enki::World;
    /// # use enki::Component;
    ///
    /// #[derive(Debug)]
    /// struct Position(f32, f32);
    /// #[derive(Debug)]
    /// struct Velocity(f32, f32);
    /// # impl Component for Position {}
    /// # impl Component for Velocity {}
    ///
    /// let mut world = World::new();
    /// let player = world.spawn_empty();
    /// world
    ///     .add_components(player, (Position(1.0, 0.0), Velocity(1.0, 2.0)))
    ///     .expect("the player should be alive");
    /// let tree = world.spawn_empty();
    /// world.add_component(tree, Position(5.0, 5.0)).expect("the tree should be alive");
    ///
    /// // Adds up the first coordinate of every position, one table at a time
    /// let mut total = 0.0;
    /// for positions in world.query_archetype::<&Position>() {
    ///     total += positions.iter().map(|position| position.0).sum::<f32>();
    /// }
    ///
    /// assert_eq!(total, 6.0);
    /// ```
    pub fn query_archetype<Q: ReadOnlyQuery>(&self) -> impl Iterator<Item = Q::Columns<'_>> {
        // Checks that `Q` does not change a component type that it also asks for elsewhere, as
        // `find_columns` requires
        let permissions = query::check_permissions::<Q>();
        let query_mask = self.find_query_mask(&permissions);
        self.find_matching_archetypes(query_mask)
            // SAFETY: `find_columns` requires three guarantees. `Q` builds only shared slices, and
            // `&self` stops any change to the world for as long as the iterator lives.
            // `check_permissions` passed above. `Q` borrows no column mutably, meaning the third
            // guarantee holds whatever other shared slices exist
            .filter_map(|archetype| unsafe { Q::find_columns(archetype) })
    }

    /// Returns an iterator with one item for each entity that has every component type in the
    /// query `Q`.
    ///
    /// The item for a query `&T` is a `&T`. For a tuple, the item is a tuple that contains the item
    /// for each field. The iterator visits the entities one table at a time, and in row order
    /// within each table.
    ///
    /// The compiler accepts `Q` only where it implements [`ReadOnlyQuery`], as `&Position` and
    /// `(&Position, &Velocity)` do. A caller passes a query that changes a component, such as
    /// `&mut Position`, to [`query_mut`](Self::query_mut) instead.
    ///
    /// The iterator borrows the world through a shared reference. As a result, the compiler rejects
    /// any change to the world while the iterator or an item is in use, and accepts other reads.
    ///
    /// # Examples
    ///
    /// The example reads every entity that has a position and a velocity, and works out where each
    /// one would move to. The query skips the tree, which has a position alone.
    ///
    /// ```
    /// use enki::World;
    /// # use enki::Component;
    ///
    /// #[derive(Debug)]
    /// struct Position(f32, f32);
    /// #[derive(Debug)]
    /// struct Velocity(f32, f32);
    /// # impl Component for Position {}
    /// # impl Component for Velocity {}
    ///
    /// let mut world = World::new();
    /// let player = world.spawn_empty();
    /// world
    ///     .add_components(player, (Position(1.0, 0.0), Velocity(1.0, 2.0)))
    ///     .expect("the player should be alive");
    /// let tree = world.spawn_empty();
    /// world.add_component(tree, Position(5.0, 5.0)).expect("the tree should be alive");
    ///
    /// // Works out where each entity that has both components would move to
    /// let targets: Vec<(f32, f32)> = world
    ///     .query::<(&Position, &Velocity)>()
    ///     .map(|(position, velocity)| (position.0 + velocity.0, position.1 + velocity.1))
    ///     .collect();
    ///
    /// assert_eq!(targets, vec![(2.0, 2.0)]);
    /// ```
    pub fn query<Q: ReadOnlyQuery>(&self) -> impl Iterator<Item = Q::Item<'_>> {
        // Splits each table's columns into one item for each entity
        self.query_archetype::<Q>().flat_map(Q::split_rows)
    }

    // ========================================================================================== \\
    //                                       Helper Methods                                       \\
    // ========================================================================================== \\

    /// Returns the mask with one set bit for each component type in `permissions`, the list that
    /// [`check_permissions`](query::check_permissions) returns for a query.
    ///
    /// Returns `None` where one of those types has no bit yet. A table stores a type only once the
    /// world has given that type a bit. As a result, `None` means that no table matches the query.
    ///
    /// # Examples
    ///
    /// With `Position` at bit 0 and `Health` at bit 1, the list for `(&Position, &mut Health)`
    /// returns the mask with bits 0 and 1 set, whether the query reads or changes each type. The
    /// list for `(&Position, &Velocity)` returns `None`, since `Velocity` has no bit.
    fn find_query_mask(&self, permissions: &[(TypeId, Permission)]) -> Option<ComponentMask> {
        // Starts with a mask that has no bits set
        let mut mask = ComponentMask::EMPTY;
        // Visits each type in the list, ignoring whether the query reads or changes it
        for (type_id, _) in permissions {
            // Finds the type's bit, and returns `None` where the world has never given it one
            let bit = self.arch_masks.get(type_id)?;
            mask |= *bit;
        }
        Some(mask)
    }

    /// Returns an iterator over the tables that have at least one entity and store every component
    /// type in `query_mask`.
    ///
    /// The iterator visits each table once at most. It yields nothing where `query_mask` is
    /// `None`, which [`find_query_mask`](Self::find_query_mask) returns for a query that asks for
    /// a type with no bit.
    ///
    /// # Examples
    ///
    /// In a world with tables for `{Position}` and `{Position, Health}`, `&Position` finds both
    /// tables, and `&Health` finds the second alone. A table that has lost its last entity matches
    /// neither. The tests `query_mut_with_several_tables_visits_only_entities_with_every_type` and
    /// `query_archetype_mut_with_emptied_table_skips_it` check both cases.
    fn find_matching_archetypes(
        &self,
        query_mask: Option<ComponentMask>,
    ) -> impl Iterator<Item = &Archetype> {
        // Filter tables by those who are non-empty and contain matching components
        self.archetypes.iter().filter(move |archetype| {
            let has_rows = archetype.len() > 0;
            let has_types = query_mask.is_some_and(|mask| archetype.has_all(mask));
            has_rows && has_types
        })
    }

    /// Returns where `entity` is stored, or `None` where the handle does not refer to a living
    /// entity.
    ///
    /// A handle does not refer to a living entity where its index lies past the last slot. It also
    /// does not refer to one where its version differs from the version in its slot, as it does for
    /// a despawned entity.
    ///
    /// # Examples
    ///
    /// For the third of three entities given a `Health` in a new world, `locate` returns table 1,
    /// row 2. Once that entity is despawned, `locate` returns `None` for the same handle.
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
    ///
    /// # Examples
    ///
    /// Adding `Health` to the table for `{Position}` creates the table for `{Position, Health}` on
    /// the first call, and finds the same table on the second. The test
    /// `add_component_with_same_types_on_two_entities_reuses_table` reaches it through
    /// `add_component`.
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

        self.find_or_create_archetype_by_mask(mask, |world| {
            // Copies the descriptions of the source table's columns
            let mut infos = world.archetypes[source].infos();
            // Selects the description of each bundle type that the entity lacks. Each other bundle
            // type replaces the component in a column that the source table already has
            let new_infos = bundle_infos
                .iter()
                .filter(|info| !source_mask.intersects(info.bit()));
            // Adds those descriptions, one for each new column in the target table
            infos.extend(new_infos);
            infos
        })
    }

    /// Returns the position in `archetypes` of the table for the types in table `source`, minus the
    /// type that `type_id` identifies. Creates that table where the world has none yet.
    ///
    /// The method builds the target mask from the bits alone, and copies the column descriptions
    /// only where it has to create the table.
    ///
    /// # Panics
    ///
    /// Panics where the world has never given a bit to the type that `type_id` identifies. A caller
    /// that has checked that table `source` stores the type never meets this case.
    ///
    /// # Examples
    ///
    /// From the table for `{Position, Health}`, removing `Health` returns the position of the table
    /// for `{Position}`. The tests `remove_component_with_existing_target_table_reuses_table` and
    /// `remove_component_with_new_target_table_creates_table` reach it through `remove_component`.
    fn find_or_create_archetype_without(&mut self, source: usize, type_id: TypeId) -> usize {
        // Finds the bit of the type being removed
        let bit = self.arch_masks[&type_id];
        // Builds the target table's mask, which is the source table's mask with that bit cleared
        let mask = self.archetypes[source].mask().without(bit);

        self.find_or_create_archetype_by_mask(mask, |world| {
            // Copies the descriptions of the source table's columns
            let mut infos = world.archetypes[source].infos();
            // Throws away the description for `type_id`, keeping every other one
            infos.retain(|info| info.type_id() != type_id);
            infos
        })
    }

    /// Returns the position in `archetypes` of the table filed under `mask`. Creates that table
    /// where the world has none yet.
    ///
    /// The method calls `build_infos` only where it has to create the table, so a lookup that finds
    /// the table copies no column descriptions. `build_infos` receives the world, and returns one
    /// description for each type in `mask`. The new table builds its mask from those descriptions,
    /// so they MUST describe exactly the types in `mask`. Otherwise `indices` would file the table
    /// under a different mask, and the next lookup for `mask` would miss it.
    ///
    /// # Panics
    ///
    /// Panics in a debug build where the descriptions from `build_infos` give the new table a mask
    /// other than `mask`.
    ///
    /// # Examples
    ///
    /// For the mask with the bits of `Position` and `Health`, the method returns the position of
    /// the table for `{Position, Health}`, and calls `build_infos` only on the first call. The tests
    /// `add_component_with_same_types_on_two_entities_reuses_table` and
    /// `remove_component_with_new_target_table_creates_table` reach it through the two methods that
    /// call it.
    fn find_or_create_archetype_by_mask(
        &mut self,
        mask: ComponentMask,
        build_infos: impl FnOnce(&Self) -> Vec<ComponentInfo>,
    ) -> usize {
        // Returns the table that already has that mask, where one exists
        if let Some(&index) = self.indices.get(&mask) {
            return index;
        }

        // Describes the new table's columns, now that the table has to be created
        let infos = build_infos(self);
        // Creates the new table, which `create_archetype` files under the mask it builds
        let index = self.create_archetype(&infos);
        debug_assert_eq!(
            self.archetypes[index].mask(),
            mask,
            "the descriptions must cover exactly the types in the mask"
        );
        index
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
    ///
    /// # Examples
    ///
    /// In a new world, the table for `{Position, Health}` takes position 1, after the empty table,
    /// and `indices` files it under the mask with bits 0 and 1 set.
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

    /// Moves `entity` from its table to the end of the table at position `target`, and calls `fill`
    /// to push the components that the target needs and the source table lacked.
    ///
    /// The method passes `take` and `fill` on to [`Archetype::move_entity`], which sets out what
    /// each closure MUST do. `take` receives each column that the target has no column for. It also
    /// updates the slot of the moved entity, and the slot of the entity that fills its old row.
    ///
    /// `fill` runs before the method updates the slots. It MUST therefore hand out any component
    /// that it replaces, for the caller to drop once the method returns, as `add_components` does.
    /// A `Drop` that panicked inside `fill` would leave both slots on their old rows.
    ///
    /// `location` MUST be where `entity` is stored now, as [`locate`](Self::locate) returns it. A
    /// stale row could lie past the end of the entity list, which panics.
    ///
    /// # Panics
    ///
    /// Panics where `target` equals `location.arch_index()`, since `get_disjoint_mut` refuses to
    /// borrow one table as both the source and the target. Also panics in the cases that
    /// [`Archetype::move_entity`] sets out, such as a `fill` that misses a new column.
    ///
    /// # Aborts
    ///
    /// The program aborts where `take` panics, such as in the `Drop` implementation of a component
    /// that it drops, as [`Archetype::move_entity`] sets out.
    ///
    /// # Examples
    ///
    /// The example moves the first of two entities out of the empty table, into a new table for
    /// `{Health}`. The second entity then fills the row that the first one left.
    ///
    /// ```ignore
    /// // Spawns two entities, which take rows 0 and 1 of the empty table
    /// let mut world = World::new();
    /// let first = world.spawn_empty();
    /// let second = world.spawn_empty();
    ///
    /// // Creates table 1, for `{Health}`, which has no rows yet
    /// let bit = world.find_or_assign_bit(TypeId::of::<Health>());
    /// let target = world.create_archetype(&[ComponentInfo::of::<Health>(bit)]);
    ///
    /// // Moves the first entity to table 1, and pushes its `Health` onto the new column
    /// let location = world.locate(first).expect("the entity should be alive");
    /// world.move_entity(first, location, target, ComponentVec::swap_remove, |table, _| {
    ///     let health_column = table.components_of_mut(TypeId::of::<Health>()).expect("a column");
    ///     health_column.push_typed(Health(100));
    /// });
    ///
    /// // Checks that the first entity occupies row 0 of table 1
    /// let moved = world.locate(first).expect("the entity should be alive");
    /// assert_eq!(moved.arch_index(), 1);
    /// assert_eq!(moved.row(), 0);
    ///
    /// // Checks that the second entity now occupies row 0 of the empty table
    /// let filled = world.locate(second).expect("the entity should be alive");
    /// assert_eq!(filled.arch_index(), World::EMPTY);
    /// assert_eq!(filled.row(), 0);
    /// ```
    fn move_entity(
        &mut self,
        entity: Entity,
        location: Location,
        target: usize,
        take: impl FnMut(&mut ComponentVec, usize),
        fill: impl FnOnce(&mut Archetype, usize),
    ) {
        // Lists the two tables to borrow, with the entity's current table first and the target
        // second
        let indices = [location.arch_index(), target];
        // Borrows both tables at once, after checking that the two positions differ
        let [source_table, target_table] = self
            .archetypes
            .get_disjoint_mut(indices)
            .expect("the target archetype should differ from the entity's own table");

        // Records the entity's row in the target, which is the target's current number of rows
        let row = target_table.len();
        // Moves the entity to the end of the target, takes its components that the target has no
        // column for, and fills its new columns, then finds the entity that fills its old row, where
        // one does
        let moved_ent = source_table.move_entity(location.row(), target_table, take, fill);

        // Describes where the entity now lives
        let new_location = Location::new(target, row);
        // Updates the slot of `entity`, and the slot of `moved_ent`, which is the entity that filled
        // its old row
        self.record_move(entity, moved_ent, location, new_location);
    }

    /// Updates the slots after `moved_ent` moves from `old_location` to `new_location`.
    ///
    /// `swapped_ent` is the entity that filled the row that `moved_ent` left, or `None` where that
    /// row was the last one in its table.
    ///
    /// The method changes only the row in the slot of `swapped_ent`. That entity stays in the table
    /// that `moved_ent` left.
    ///
    /// # Examples
    ///
    /// Where the first of two entities in the empty table moves to row 3 of table 1, the second
    /// entity fills row 0. `record_move` then points the first slot at table 1, row 3, and the
    /// second slot at row 0 of the empty table. The test
    /// `despawn_with_first_row_updates_location_of_filling_entity` reaches it through
    /// `despawn`.
    fn record_move(
        &mut self,
        moved_ent: Entity,
        swapped_ent: Option<Entity>,
        old_location: Location,
        new_location: Location,
    ) {
        // Points the slot of the entity that filled the old row at that row, where one did
        self.record_filled_row(swapped_ent, old_location.row());
        // Points the moved entity's slot at its new table and row
        self.slots[moved_ent.index()].location = new_location;
    }

    /// Points the slot of `filled_ent` at row `row`, where `filled_ent` is the entity that a
    /// swap-remove moved into that row. Does nothing where `filled_ent` is `None`, as it is when the
    /// removed row was the last one in its table.
    ///
    /// The entity stays in the same table, so the method changes only the row in its slot.
    ///
    /// # Examples
    ///
    /// Where the first of three entities leaves row 0 of its table, the third entity fills row 0.
    /// `record_filled_row(Some(third), 0)` then points the slot of the third entity at row 0, and
    /// `record_filled_row(None, 0)` changes nothing. The test
    /// `despawn_with_first_row_updates_location_of_filling_entity` reaches it through `despawn`,
    /// and `add_components_with_entity_leaving_first_row_updates_filling_entity` through a move.
    fn record_filled_row(&mut self, filled_ent: Option<Entity>, row: usize) {
        if let Some(filled) = filled_ent {
            self.slots[filled.index()].location.set_row(row);
        }
    }
}

impl Default for World {
    /// Returns a world with no entities and no component types, as [`World::new`] does.
    ///
    /// The example on [`World`] builds the same world with [`World::new`].
    fn default() -> Self {
        Self::new() // Builds the same empty world as `new`
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
    /// For example, once a caller despawns the first entity in a slot, the slot stores version 1.
    version: u32,
    /// Records where the entity is stored. In a free slot, the value is out of date until
    /// `spawn_empty` overwrites it.
    ///
    /// The example on [`Location`] shows a value.
    location: Location,
}

// Stops the build where the size of a slot differs from 12 bytes. A slot needs 4 bytes for its
// version, and 4 bytes for each of its two location fields
const _: () = assert!(size_of::<Slot>() == 12);

/// Records the table and the row where one entity is stored.
///
/// # Examples
///
/// `Location::new(1, 2)` records row 2 of table 1. After `set_row(0)`, `arch_index` still returns
/// 1, and `row` returns 0. The test `set_row_with_new_row_keeps_table` checks it.
#[derive(Clone, Copy, Debug)]
struct Location {
    /// Stores the position in `World::archetypes` of the table that contains the entity.
    ///
    /// For example, `Location::new(1, 2)` stores 1.
    arch_index: u32,
    /// Stores the row that the entity occupies in that table.
    ///
    /// For example, `Location::new(1, 2)` stores 2.
    row: u32,
}

impl Location {
    /// Returns the location of row `row` in the table at position `arch_index`.
    ///
    /// The example on [`Location`] shows a call.
    ///
    /// # Panics
    ///
    /// Panics where either value exceeds `u32::MAX`.
    ///
    /// # Examples
    ///
    /// The test `location_new_with_row_past_u32_max_panics` passes a row one past `u32::MAX`, which
    /// panics, since the row does not fit in a `u32`.
    fn new(arch_index: usize, row: usize) -> Self {
        // Narrows the table's position to a `u32`, and panics where it does not fit
        let arch_index =
            u32::try_from(arch_index).expect("a world should have fewer than 2^32 tables");
        // Narrows the row to a `u32`, which always fits, since a world has at most 2^32 entities
        let row = u32::try_from(row).expect("a table should have fewer than 2^32 rows");
        Self { arch_index, row }
    }

    /// Returns the position in `World::archetypes` of the table that contains the entity.
    ///
    /// The example on [`Location`] shows a call.
    fn arch_index(self) -> usize {
        self.arch_index as usize
    }

    /// Returns the row that the entity occupies in its table.
    ///
    /// The example on [`Location`] shows a call.
    fn row(self) -> usize {
        self.row as usize
    }

    /// Changes the row to `row`, and keeps the table.
    ///
    /// The example on [`Location`] shows a call.
    ///
    /// # Panics
    ///
    /// Panics where `row` exceeds `u32::MAX`.
    fn set_row(&mut self, row: usize) {
        *self = Self::new(self.arch_index(), row);
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use std::array;
    use std::collections::HashSet;
    use std::env;
    use std::panic::{self, AssertUnwindSafe};
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, PartialEq)]
    struct Position(f32);

    impl Component for Position {}

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    impl Component for Health {}

    #[derive(Debug, PartialEq)]
    struct Velocity(f32);

    impl Component for Velocity {}

    /// Takes no memory, which gives the world a column for a type with a size of zero.
    #[derive(Debug, PartialEq)]
    struct Marker;

    impl Component for Marker {}

    /// Stands for a different component type at each value of `N`, which lets one test build a
    /// bundle or a query of thirty-two types.
    #[derive(Debug, PartialEq)]
    struct Numbered<const N: usize>(usize);

    impl<const N: usize> Component for Numbered<N> {}

    /// Defines the bundle type with the most fields that `Bundle` is implemented for.
    type Bundle32 = (
        Numbered<0>,
        Numbered<1>,
        Numbered<2>,
        Numbered<3>,
        Numbered<4>,
        Numbered<5>,
        Numbered<6>,
        Numbered<7>,
        Numbered<8>,
        Numbered<9>,
        Numbered<10>,
        Numbered<11>,
        Numbered<12>,
        Numbered<13>,
        Numbered<14>,
        Numbered<15>,
        Numbered<16>,
        Numbered<17>,
        Numbered<18>,
        Numbered<19>,
        Numbered<20>,
        Numbered<21>,
        Numbered<22>,
        Numbered<23>,
        Numbered<24>,
        Numbered<25>,
        Numbered<26>,
        Numbered<27>,
        Numbered<28>,
        Numbered<29>,
        Numbered<30>,
        Numbered<31>,
    );

    /// Defines the query type with the most fields that `Query` is implemented for. It reads
    /// `Numbered<0>` to `Numbered<30>`, and changes `Numbered<31>`.
    type Query32 = (
        &'static Numbered<0>,
        &'static Numbered<1>,
        &'static Numbered<2>,
        &'static Numbered<3>,
        &'static Numbered<4>,
        &'static Numbered<5>,
        &'static Numbered<6>,
        &'static Numbered<7>,
        &'static Numbered<8>,
        &'static Numbered<9>,
        &'static Numbered<10>,
        &'static Numbered<11>,
        &'static Numbered<12>,
        &'static Numbered<13>,
        &'static Numbered<14>,
        &'static Numbered<15>,
        &'static Numbered<16>,
        &'static Numbered<17>,
        &'static Numbered<18>,
        &'static Numbered<19>,
        &'static Numbered<20>,
        &'static Numbered<21>,
        &'static Numbered<22>,
        &'static Numbered<23>,
        &'static Numbered<24>,
        &'static Numbered<25>,
        &'static Numbered<26>,
        &'static Numbered<27>,
        &'static Numbered<28>,
        &'static Numbered<29>,
        &'static Numbered<30>,
        &'static mut Numbered<31>,
    );

    /// Adds 1 to a shared counter when dropped, which shows how many times clean-up code ran.
    #[derive(Debug)]
    struct DropCounter {
        drops: Arc<AtomicUsize>,
    }

    impl Component for DropCounter {}

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Panics when dropped where `panics` is true, which stands for clean-up code that fails. The
    /// `id` tells one entity's component apart from another's.
    #[derive(Debug)]
    struct Volatile {
        id: u32,
        panics: bool,
    }

    impl Component for Volatile {}

    impl Drop for Volatile {
        fn drop(&mut self) {
            if self.panics {
                panic!("the volatile component {} failed to clean up", self.id);
            }
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

    /// Returns a `Bundle32` whose field of type `Numbered<N>` contains `N`.
    fn build_bundle_32() -> Bundle32 {
        (
            Numbered(0),
            Numbered(1),
            Numbered(2),
            Numbered(3),
            Numbered(4),
            Numbered(5),
            Numbered(6),
            Numbered(7),
            Numbered(8),
            Numbered(9),
            Numbered(10),
            Numbered(11),
            Numbered(12),
            Numbered(13),
            Numbered(14),
            Numbered(15),
            Numbered(16),
            Numbered(17),
            Numbered(18),
            Numbered(19),
            Numbered(20),
            Numbered(21),
            Numbered(22),
            Numbered(23),
            Numbered(24),
            Numbered(25),
            Numbered(26),
            Numbered(27),
            Numbered(28),
            Numbered(29),
            Numbered(30),
            Numbered(31),
        )
    }

    /// Panics where the slots, the tables and `indices` disagree with each other.
    ///
    /// A slot is live where `free_slots` lacks its index and its version is below `u32::MAX`. The
    /// helper checks five things:
    ///
    /// - Each live slot points at a table row that holds the slot's own entity.
    /// - Each table row holds an entity whose slot is live and points back at that row. Together
    ///   with the first check and equal counts, every live slot then owns exactly one row.
    /// - Every column has one component for each row of its table.
    /// - `indices` files each table under the table's own mask, and has one entry for each table.
    /// - `free_slots` lists each index once, and only indices of slots that are neither live nor
    ///   retired.
    fn check_invariants(world: &World) {
        // Collects the freed indices, and panics where one is freed twice or has no slot
        let mut freed = HashSet::new();
        for &index in &world.free_slots {
            let index = index as usize;
            assert!(index < world.slots.len(), "freed index {index} has no slot");
            assert!(freed.insert(index), "index {index} freed twice");
        }

        // Checks that each live slot points at a row that holds its entity, and counts them
        let mut live_slots = 0;
        for (index, slot) in world.slots.iter().enumerate() {
            let retired = slot.version == u32::MAX;
            let free = freed.contains(&index);
            assert!(!(retired && free), "retired slot {index} in the free slots");
            if retired || free {
                continue;
            }
            live_slots += 1;
            let entity = Entity::new(index as u32, slot.version);
            let table = &world.archetypes[slot.location.arch_index()];
            let found = table.entities().get(slot.location.row());
            assert_eq!(found, Some(&entity), "entity at the row of slot {index}");
        }

        let mut rows = 0;
        for (position, table) in world.archetypes.iter().enumerate() {
            // Checks that each row's entity has a live slot that points back at this row
            for (row, entity) in table.entities().iter().enumerate() {
                let slot = world.slots[entity.index()];
                let place = (slot.location.arch_index(), slot.location.row());
                assert!(!freed.contains(&entity.index()), "freed slot of {entity:?}");
                assert_eq!(
                    slot.version,
                    entity.version(),
                    "version in slot of {entity:?}"
                );
                assert_eq!(place, (position, row), "location in slot of {entity:?}");
            }
            rows += table.len();

            // Checks that every column has one component for each row
            for info in table.infos() {
                let column = table
                    .components_of(info.type_id())
                    .expect("the table should have a column for each of its types");
                assert_eq!(
                    column.len(),
                    table.len(),
                    "column length in table {position}"
                );
            }

            // Checks that `indices` files the table under its own mask
            let filed = world.indices.get(&table.mask());
            assert_eq!(
                filed,
                Some(&position),
                "position under the mask of table {position}"
            );
        }

        assert_eq!(rows, live_slots, "rows across every table");
        assert_eq!(
            world.indices.len(),
            world.archetypes.len(),
            "masks in `indices`"
        );
    }

    /// Checks that `new` and `default` each build a world with one table with no columns, and no
    /// entities or component types
    #[test]
    fn new_and_default_with_no_args_return_world_with_only_empty_table() {
        type Build = fn() -> World;
        let cases: [(&str, Build); 2] = [("new", World::new), ("default", World::default)];

        for (label, build) in cases {
            let world = build();

            let empty_table = &world.archetypes[World::EMPTY];
            let empty_index = world.indices.get(&ComponentMask::EMPTY);
            assert_eq!(world.archetypes.len(), 1, "number of tables from {label}");
            assert_eq!(
                empty_table.mask(),
                ComponentMask::EMPTY,
                "mask of the empty table from {label}"
            );
            assert_eq!(empty_table.len(), 0, "rows in the empty table from {label}");
            assert_eq!(
                empty_index,
                Some(&World::EMPTY),
                "table under the empty mask from {label}"
            );
            assert!(world.arch_masks.is_empty(), "component types from {label}");
            assert!(world.slots.is_empty(), "slots from {label}");
            assert!(world.free_slots.is_empty(), "free slots from {label}");
        }
    }

    /// Checks that a world with no freed slots hands out indices in order, each at version 0
    #[test]
    fn spawn_empty_with_varying_counts_returns_sequential_handles_at_version_zero() {
        let cases = [1_u32, 2, 5];

        for count in cases {
            let mut world = World::new();

            let entities: Vec<Entity> = (0..count).map(|_| world.spawn_empty()).collect();

            let expected: Vec<Entity> = (0..count).map(|index| Entity::new(index, 0)).collect();
            let rows = world.archetypes[World::EMPTY].len();
            assert_eq!(entities, expected, "handles for {count} spawns");
            assert_eq!(rows, count as usize, "rows for {count} spawns");
        }
    }

    /// Checks that each spawned entity lives in the empty table, at the row of its spawn order
    #[test]
    fn spawn_empty_with_several_spawns_places_each_in_next_row_of_empty_table() {
        let mut world = World::new();

        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];

        for (row, entity) in entities.into_iter().enumerate() {
            let location = locate_alive(&world, entity);
            assert_eq!(location.arch_index(), World::EMPTY, "table of entity {row}");
            assert_eq!(location.row(), row, "row of entity {row}");
        }
    }

    /// Checks that a freed slot is reused, at a version one above the dead entity's
    #[test]
    fn spawn_empty_with_freed_slot_reuses_index_at_raised_version() {
        let mut world = World::new();
        let dead = world.spawn_empty();
        world.despawn(dead);

        let reborn = world.spawn_empty();

        assert_eq!(reborn, Entity::new(0, 1), "handle of the new entity");
        assert_eq!(world.slots.len(), 1, "slots");
        assert!(world.free_slots.is_empty(), "free slots");
    }

    /// Checks that freed slots come back last freed first, since `free_slots` is a stack
    #[test]
    fn spawn_empty_with_several_freed_slots_reuses_last_freed_first() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world.spawn_empty();
        world.despawn(first);
        world.despawn(second);

        let reborn = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];

        assert_eq!(
            reborn,
            [Entity::new(1, 1), Entity::new(0, 1), Entity::new(3, 0)]
        );
    }

    /// Checks that a despawned entity is gone, and that the call reports that it existed
    #[test]
    fn despawn_with_living_entity_returns_true_and_forgets_entity() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(10))
            .expect("the entity should be alive");

        let despawned = world.despawn(entity);

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
    fn despawn_with_handle_of_no_living_entity_returns_false() {
        let cases = [
            ("despawned", Entity::new(0, 0)),
            ("index past the last slot", Entity::new(5, 0)),
            ("later version", Entity::new(0, 7)),
            ("largest index and version", Entity::new(u32::MAX, u32::MAX)),
        ];

        for (label, handle) in cases {
            let mut world = World::new();
            let entity = world.spawn_empty();
            world.despawn(entity);

            let despawned = world.despawn(handle);

            assert!(!despawned, "answer for {label}");
            assert_eq!(world.free_slots, [0], "free slots for {label}");
        }
    }

    /// Checks that the entity that fills the empty row can still be found, with its own component
    #[test]
    fn despawn_with_first_row_updates_location_of_filling_entity() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }
        let [first, second, third] = entities;

        world.despawn(first);

        assert_eq!(
            locate_alive(&world, third).row(),
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
    fn despawn_with_drop_counter_drops_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let doomed = world.spawn_empty();
        let survivor = world.spawn_empty();
        world
            .add_component(doomed, build_counter(&drops))
            .expect("the entity should be alive");
        world
            .add_component(survivor, build_counter(&drops))
            .expect("the entity should be alive");

        world.despawn(doomed);

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that a slot whose version reaches `u32::MAX` is never reused, since its next version
    /// would wrap round to one that an old handle may still have
    #[test]
    fn despawn_with_version_reaching_max_retires_slot() {
        let mut world = World::new();
        world.spawn_empty();
        world.slots[0].version = u32::MAX - 1;
        let last_version = Entity::new(0, u32::MAX - 1);

        let despawned = world.despawn(last_version);
        let next = world.spawn_empty();

        assert!(despawned, "answer for the entity at the last version");
        assert_eq!(
            world.slots[0].version,
            u32::MAX,
            "version of the retired slot"
        );
        assert_eq!(next, Entity::new(1, 0), "handle of the next entity");
    }

    /// Checks that a component whose clean-up code panics ends the program during a despawn,
    /// rather than passing the panic on to the caller
    ///
    /// An abort ends the whole test process, which no test can catch. The test therefore runs the
    /// test binary again as a child process, for this one test, with `ENKI_DESPAWN_ABORTS` set.
    /// The child sees the variable and despawns the entity, which aborts it. The parent then
    /// checks how the child ended.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn despawn_with_panicking_drop_aborts() {
        const CHILD_VARIABLE: &str = "ENKI_DESPAWN_ABORTS";
        const TEST_PATH: &str = "world::tests::despawn_with_panicking_drop_aborts";

        // Despawns the entity in the child process, which should end the child at this call
        if env::var_os(CHILD_VARIABLE).is_some() {
            let mut world = World::new();
            let entity = world.spawn_empty();
            let volatile = Volatile {
                id: 1,
                panics: true,
            };
            world
                .add_component(entity, volatile)
                .expect("the entity should be alive");
            world.despawn(entity);
            return;
        }

        let test_binary = env::current_exe().expect("the test binary should have a path");

        let output = Command::new(test_binary)
            .args(["--exact", TEST_PATH, "--nocapture"])
            .env(CHILD_VARIABLE, "1")
            .output()
            .expect("the test binary should start as a child process");

        // A test that fails through an ordinary panic ends the test binary with exit code 101
        assert!(!output.status.success(), "child should fail");
        assert_ne!(output.status.code(), Some(101), "child exit code");
    }

    /// Checks that a first component moves the entity out of the empty table, into a new table
    #[test]
    fn add_component_with_new_component_moves_entity_to_new_table() {
        let mut world = World::new();
        let entity = world.spawn_empty();

        world
            .add_component(entity, Position(1.5))
            .expect("the entity should be alive");

        let location = locate_alive(&world, entity);
        let bit = world.arch_masks[&TypeId::of::<Position>()];
        let mask = world.archetypes[location.arch_index()].mask();
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(location.arch_index(), 1, "table of the entity");
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
    fn add_component_with_second_component_keeps_first_component() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.5))
            .expect("the entity should be alive");

        world
            .add_component(entity, Health(100))
            .expect("the entity should be alive");

        let location = locate_alive(&world, entity);
        let mask = world.archetypes[location.arch_index()].mask();
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
    fn add_component_with_existing_component_type_replaces_value_in_place() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");
        let before = locate_alive(&world, entity);

        world
            .add_component(entity, Health(2))
            .expect("the entity should be alive");

        let after = locate_alive(&world, entity);
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            after.arch_index(),
            before.arch_index(),
            "table of the entity"
        );
        assert_eq!(after.row(), before.row(), "row of the entity");
        assert_eq!(world.get::<Health>(entity), Some(&Health(2)), "component");
    }

    /// Checks that replacing a component drops the old one, and leaves the new one alive
    #[test]
    fn add_component_with_existing_component_type_drops_old_value_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, build_counter(&drops))
            .expect("the entity should be alive");

        world
            .add_component(entity, build_counter(&drops))
            .expect("the entity should be alive");

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that entities with the same component types share one table
    #[test]
    fn add_component_with_same_types_on_two_entities_reuses_table() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world
            .add_component(first, Health(1))
            .expect("the entity should be alive");

        world
            .add_component(second, Health(2))
            .expect("the entity should be alive");

        let first_location = locate_alive(&world, first);
        let second_location = locate_alive(&world, second);
        let rows = [first_location.row(), second_location.row()];
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            first_location.arch_index(),
            second_location.arch_index(),
            "shared table"
        );
        assert_eq!(rows, [0, 1], "rows");
    }

    /// Checks that an entity ends in the same table, whichever order its components arrive in
    #[test]
    fn add_component_with_types_in_either_order_ends_in_same_table() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world
            .add_component(first, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(first, Health(1))
            .expect("the entity should be alive");

        world
            .add_component(second, Health(2))
            .expect("the entity should be alive");
        world
            .add_component(second, Position(2.0))
            .expect("the entity should be alive");

        let first_table = locate_alive(&world, first).arch_index();
        let second_table = locate_alive(&world, second).arch_index();
        let masks = world.indices.len();
        assert_eq!(first_table, second_table, "table of each entity");
        assert_eq!(masks, world.archetypes.len(), "one mask for each table");
    }

    /// Checks that moving one entity leaves every other entity with its own components
    #[test]
    fn add_component_with_entity_leaving_middle_row_keeps_other_components_in_place() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }
        let [first, second, third] = entities;

        world
            .add_component(second, Position(9.0))
            .expect("the entity should be alive");

        assert_eq!(
            locate_alive(&world, third).row(),
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
    fn add_component_with_drop_counter_moving_tables_drops_nothing() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, build_counter(&drops))
            .expect("the entity should be alive");

        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        assert_eq!(drops.load(Ordering::Relaxed), 0);
    }

    /// Checks that a component type that takes no memory still moves the entity to its own table
    #[test]
    fn add_component_with_zero_size_type_moves_entity_to_new_table() {
        #[derive(Debug)]
        struct Tag;

        impl Component for Tag {}

        let mut world = World::new();
        let entity = world.spawn_empty();

        world
            .add_component(entity, Tag)
            .expect("the entity should be alive");

        assert_eq!(
            locate_alive(&world, entity).arch_index(),
            1,
            "table of the entity"
        );
        assert!(world.get::<Tag>(entity).is_some(), "component");
    }

    /// Checks that `add_component` panics on a 65th component type, since a mask has room for 64
    /// bits
    #[test]
    #[should_panic]
    fn add_component_with_65th_component_type_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        for type_id in build_64_type_ids() {
            world.find_or_assign_bit(type_id);
        }

        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");
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
        assert_eq!(world.arch_masks.len(), 2, "component types");
    }

    /// Checks that removing a component hands it back, and keeps the entity's other components
    #[test]
    fn remove_component_with_stored_component_returns_component_and_keeps_other_components() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.5))
            .expect("the entity should be alive");
        world
            .add_component(entity, Health(100))
            .expect("the entity should be alive");

        let removed = world.remove_component::<Health>(entity);

        let location = locate_alive(&world, entity);
        let position_bit = world.arch_masks[&TypeId::of::<Position>()];
        let mask = world.archetypes[location.arch_index()].mask();
        assert_eq!(removed, Some(Health(100)), "returned component");
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
    fn remove_component_with_only_component_returns_entity_to_empty_table() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        world.remove_component::<Health>(entity);

        let location = locate_alive(&world, entity);
        assert_eq!(location.arch_index(), World::EMPTY, "table of the entity");
        assert_eq!(world.archetypes.len(), 2, "number of tables");
    }

    /// Checks that removal finds the table that `add_component` built, rather than building a second
    /// one
    #[test]
    fn remove_component_with_existing_target_table_reuses_table() {
        let mut world = World::new();
        let reference = world.spawn_empty();
        let entity = world.spawn_empty();
        world
            .add_component(reference, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(entity, Position(2.0))
            .expect("the entity should be alive");
        world
            .add_component(entity, Health(2))
            .expect("the entity should be alive");

        world.remove_component::<Health>(entity);

        let reference_table = locate_alive(&world, reference).arch_index();
        let entity_table = locate_alive(&world, entity).arch_index();
        assert_eq!(entity_table, reference_table, "table of each entity");
        assert_eq!(world.archetypes.len(), 3, "number of tables");
    }

    /// Checks that removal reaches a table that `add_component` never built, and builds it
    #[test]
    fn remove_component_with_new_target_table_creates_table() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        world.remove_component::<Position>(entity);

        let location = locate_alive(&world, entity);
        let health_bit = world.arch_masks[&TypeId::of::<Health>()];
        let mask = world.archetypes[location.arch_index()].mask();
        assert_eq!(world.archetypes.len(), 4, "number of tables");
        assert_eq!(mask, health_bit, "mask of the new table");
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(1)),
            "kept component"
        );
    }

    /// Checks that `remove_component` returns `None` for a missing type, and for an entity that is
    /// not alive
    #[test]
    fn remove_component_with_nothing_to_remove_returns_none() {
        let mut world = World::new();
        let bare = world.spawn_empty();
        let with_position = world.spawn_empty();
        let despawned = world.spawn_empty();
        world
            .add_component(with_position, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(despawned, Health(1))
            .expect("the entity should be alive");
        world.despawn(despawned);

        let answers = [
            (
                "entity with no components",
                world.remove_component::<Health>(bare),
            ),
            (
                "entity with another type",
                world.remove_component::<Health>(with_position),
            ),
            (
                "despawned entity",
                world.remove_component::<Health>(despawned),
            ),
        ];

        let kept = world.get::<Position>(with_position);
        for (label, removed) in answers {
            assert!(removed.is_none(), "answer for {label}");
        }
        assert_eq!(kept, Some(&Position(1.0)), "untouched component");
    }

    /// Checks that `remove_component` returns `None` for a type that the world has never met,
    /// without giving that type a bit or touching the entity's other components
    #[test]
    fn remove_component_with_type_never_seen_returns_none() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");

        let removed = world.remove_component::<Velocity>(entity);

        let velocity_bit = world.arch_masks.get(&TypeId::of::<Velocity>());
        assert_eq!(removed, None, "removed component");
        assert_eq!(velocity_bit, None, "bit for `Velocity`");
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.0)),
            "untouched component"
        );
    }

    /// Checks that removing a component drops it once, and does not drop anything else
    #[test]
    fn remove_component_with_drop_counter_drops_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, build_counter(&drops))
            .expect("the entity should be alive");
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        world.remove_component::<DropCounter>(entity);

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that moving one entity out leaves the entity that fills its row with its own
    /// components
    #[test]
    fn remove_component_with_entity_leaving_first_row_keeps_other_components_in_place() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        for (value, entity) in [first, second].into_iter().enumerate() {
            world
                .add_component(entity, Position(value as f32))
                .expect("the entity should be alive");
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }

        world.remove_component::<Health>(first);

        assert_eq!(
            locate_alive(&world, second).row(),
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
        let stored = world.spawn_empty();
        let bare = world.spawn_empty();
        let despawned = world.spawn_empty();
        world
            .add_component(stored, Health(5))
            .expect("the entity should be alive");
        world
            .add_component(despawned, Health(6))
            .expect("the entity should be alive");
        world.despawn(despawned);

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
        let stale = world.spawn_empty();
        world
            .add_component(stale, Health(1))
            .expect("the entity should be alive");
        world.despawn(stale);
        let reborn = world.spawn_empty();
        world
            .add_component(reborn, Health(2))
            .expect("the entity should be alive");

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
        let changed = world.spawn_empty();
        let untouched = world.spawn_empty();
        world
            .add_component(changed, Health(1))
            .expect("the entity should be alive");
        world
            .add_component(untouched, Health(1))
            .expect("the entity should be alive");

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
        let bare = world.spawn_empty();
        let despawned = world.spawn_empty();
        world
            .add_component(despawned, Health(1))
            .expect("the entity should be alive");
        world.despawn(despawned);

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
        let alone = world.spawn_empty();
        let paired = world.spawn_empty();
        world
            .add_component(alone, build_counter(&drops))
            .expect("the entity should be alive");
        world
            .add_component(paired, build_counter(&drops))
            .expect("the entity should be alive");
        world
            .add_component(paired, Health(1))
            .expect("the entity should be alive");

        drop(world);

        assert_eq!(drops.load(Ordering::Relaxed), 2);
    }

    /// Checks that a type of size zero with clean-up code is dropped once, whether it moves to
    /// another table, is removed, is despawned, or stays until the world is dropped
    #[test]
    fn drop_with_zero_size_drop_type_drops_each_component_once() {
        // Counts the drops outside the type, since a type of size zero has no room for a counter
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct Fleeting;

        impl Component for Fleeting {}

        impl Drop for Fleeting {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut world = World::new();
        let mover = world.spawn_empty();
        let removed = world.spawn_empty();
        let despawned = world.spawn_empty();
        for entity in [mover, removed, despawned] {
            world
                .add_component(entity, Fleeting)
                .expect("the entity should be alive");
        }

        world
            .add_component(mover, Health(1))
            .expect("the entity should be alive");
        let drops_after_move = DROPS.load(Ordering::Relaxed);
        world.remove_component::<Fleeting>(removed);
        world.despawn(despawned);
        let drops_after_removals = DROPS.load(Ordering::Relaxed);
        drop(world);
        let drops_after_world = DROPS.load(Ordering::Relaxed);

        assert_eq!(drops_after_move, 0, "drops caused by the move");
        assert_eq!(
            drops_after_removals, 2,
            "drops after `remove_component` and `despawn`"
        );
        assert_eq!(drops_after_world, 3, "drops after the world is gone");
    }

    /// Checks that despawning the entity in the last row leaves every other entity in its row
    #[test]
    fn despawn_with_last_row_leaves_other_rows_in_place() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }
        let [first, second, third] = entities;

        let despawned = world.despawn(third);

        let table = locate_alive(&world, first).arch_index();
        let rows = [
            locate_alive(&world, first).row(),
            locate_alive(&world, second).row(),
        ];
        assert!(despawned, "answer for the entity in the last row");
        assert_eq!(rows, [0, 1], "rows of the other entities");
        assert_eq!(world.archetypes[table].len(), 2, "rows in the table");
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
    }

    /// Checks that `add_component` hands the component back for a handle of no living entity, and
    /// leaves the world unchanged
    #[test]
    fn add_component_with_handle_of_no_living_entity_returns_component() {
        let cases = [
            ("despawned entity", Entity::new(1, 0)),
            ("index past the last slot", Entity::new(5, 0)),
            ("later version", Entity::new(0, 1)),
            ("largest index and version", Entity::new(u32::MAX, u32::MAX)),
        ];

        for (label, handle) in cases {
            let mut world = World::new();
            world.spawn_empty();
            // Spawns and despawns the entity at index 1, which the first case then names
            let despawned = world.spawn_empty();
            world.despawn(despawned);

            let added = world.add_component(handle, Health(1));

            assert_eq!(added, Err(Health(1)), "answer for {label}");
            assert_eq!(world.archetypes.len(), 1, "number of tables for {label}");
            assert!(
                world.arch_masks.is_empty(),
                "component types for {label}"
            );
        }
    }

    /// Checks that a bundle of new types moves the entity to one new table, with every component
    #[test]
    fn add_components_with_new_types_moves_entity_to_new_table() {
        let mut world = World::new();
        let entity = world.spawn_empty();

        let added = world.add_components(entity, (Position(1.5), Health(100)));

        let location = locate_alive(&world, entity);
        let mask = world.archetypes[location.arch_index()].mask();
        let empty_rows = world.archetypes[World::EMPTY].len();
        assert_eq!(added, Ok(()), "answer for a living entity");
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            mask,
            ComponentMask::from_positions(&[0, 1]),
            "mask of the entity's table"
        );
        assert_eq!(empty_rows, 0, "rows in the empty table");
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.5)),
            "position"
        );
        assert_eq!(world.get::<Health>(entity), Some(&Health(100)), "health");
    }

    /// Checks that `add_components` hands the bundle back for a handle of no living entity, and
    /// leaves the world unchanged
    #[test]
    fn add_components_with_handle_of_no_living_entity_returns_bundle() {
        let cases = [
            ("despawned", Entity::new(0, 0)),
            ("index past the last slot", Entity::new(5, 0)),
            ("later version", Entity::new(0, 7)),
        ];

        for (label, handle) in cases {
            let mut world = World::new();
            let entity = world.spawn_empty();
            world.despawn(entity);

            let added = world.add_components(handle, (Position(1.0), Health(1)));

            assert_eq!(added, Err((Position(1.0), Health(1))), "answer for {label}");
            assert_eq!(world.archetypes.len(), 1, "number of tables for {label}");
            assert!(
                world.arch_masks.is_empty(),
                "component types for {label}"
            );
        }
    }

    /// Checks that `add_components` hands the bundle back without dropping it, where the handle
    /// refers to no living entity, and that the bundle drops once when the caller drops it
    #[test]
    fn add_components_with_despawned_entity_returns_bundle_undropped() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world.despawn(entity);

        let returned = world.add_components(entity, (build_counter(&drops), Health(1)));
        let drops_while_held = drops.load(Ordering::Relaxed);
        drop(returned);
        let drops_after = drops.load(Ordering::Relaxed);

        assert_eq!(drops_while_held, 0, "drops while the caller holds it");
        assert_eq!(drops_after, 1, "drops once the caller drops it");
    }

    /// Checks that a bundle of types that the entity already has replaces each value in place
    #[test]
    fn add_components_with_every_type_stored_replaces_values_in_place() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, (Position(1.0), Health(1)))
            .expect("the entity should be alive");
        let before = locate_alive(&world, entity);

        let added = world.add_components(entity, (Health(2), Position(2.0)));

        let after = locate_alive(&world, entity);
        assert_eq!(added, Ok(()), "answer for a living entity");
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            after.arch_index(),
            before.arch_index(),
            "table of the entity"
        );
        assert_eq!(after.row(), before.row(), "row of the entity");
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(2.0)),
            "position"
        );
        assert_eq!(world.get::<Health>(entity), Some(&Health(2)), "health");
    }

    /// Checks that replacing every component in place drops each old value once, and nothing else
    #[test]
    fn add_components_with_every_type_stored_drops_old_values_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, (build_counter(&drops), Health(1)))
            .expect("the entity should be alive");

        world
            .add_components(entity, (build_counter(&drops), Health(2)))
            .expect("the entity should be alive");

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that a bundle of new and stored types adds the new ones, replaces the stored ones, and
    /// keeps every other component
    #[test]
    fn add_components_with_new_and_stored_types_keeps_one_value_per_type() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, (Position(1.0), Health(1)))
            .expect("the entity should be alive");

        let added = world.add_components(entity, (Health(5), Velocity(2.0)));

        let location = locate_alive(&world, entity);
        let mask = world.archetypes[location.arch_index()].mask();
        assert_eq!(added, Ok(()), "answer for a living entity");
        assert_eq!(world.archetypes.len(), 3, "number of tables");
        assert_eq!(
            mask,
            ComponentMask::from_positions(&[0, 1, 2]),
            "mask of the entity's table"
        );
        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position(1.0)),
            "kept component"
        );
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(5)),
            "replaced component"
        );
        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity(2.0)),
            "new component"
        );
    }

    /// Checks that a bundle that moves the entity drops only the component that it replaces
    #[test]
    fn add_components_with_new_and_stored_types_drops_only_replaced_value() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, build_counter(&drops))
            .expect("the entity should be alive");

        world
            .add_components(entity, (build_counter(&drops), Health(1)))
            .expect("the entity should be alive");

        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    /// Checks that a bundle that moves the entity, and replaces a component whose `Drop` panics,
    /// leaves each handle on its own entity once the caller catches the panic
    #[test]
    fn add_components_with_panicking_replaced_component_keeps_each_handle_on_its_own_entity() {
        let mut world = World::new();
        let moving = world.spawn_empty();
        let filling = world.spawn_empty();
        world
            .add_components(moving, (Volatile { id: 1, panics: true }, Position(1.0)))
            .expect("the entity should be alive");
        world
            .add_components(filling, (Volatile { id: 2, panics: false }, Position(2.0)))
            .expect("the entity should be alive");

        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            world.add_components(moving, (Volatile { id: 3, panics: false }, Velocity(9.0)))
        }));

        let read_volatile = |entity| world.get::<Volatile>(entity).map(|volatile| volatile.id);
        assert!(outcome.is_err(), "panic from the replaced component");
        assert_eq!(read_volatile(moving), Some(3), "moved entity's replacement");
        assert_eq!(
            world.get::<Position>(moving),
            Some(&Position(1.0)),
            "moved entity's kept component"
        );
        assert_eq!(
            world.get::<Velocity>(moving),
            Some(&Velocity(9.0)),
            "moved entity's new component"
        );
        assert_eq!(read_volatile(filling), Some(2), "filling entity's component");
        assert_eq!(
            world.get::<Position>(filling),
            Some(&Position(2.0)),
            "filling entity's position"
        );
    }

    /// Checks that the entity that fills the row left behind keeps its own component, at that row
    #[test]
    fn add_components_with_entity_leaving_first_row_updates_filling_entity() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }
        let [first, second, third] = entities;

        world
            .add_components(first, (Position(9.0),))
            .expect("the entity should be alive");

        assert_eq!(
            locate_alive(&world, third).row(),
            0,
            "row of the filling entity"
        );
        assert_eq!(
            world.get::<Health>(first),
            Some(&Health(0)),
            "moved entity's health"
        );
        assert_eq!(
            world.get::<Position>(first),
            Some(&Position(9.0)),
            "moved entity's new component"
        );
        assert_eq!(
            world.get::<Health>(second),
            Some(&Health(1)),
            "second entity's component"
        );
        assert_eq!(
            world.get::<Health>(third),
            Some(&Health(2)),
            "filling entity's component"
        );
    }

    /// Checks that two bundles with the same types in either order reach the same table
    #[test]
    fn add_components_with_types_in_either_order_ends_in_same_table() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();

        world
            .add_components(first, (Position(1.0), Health(1)))
            .expect("the entity should be alive");
        world
            .add_components(second, (Health(2), Position(2.0)))
            .expect("the entity should be alive");

        let first_table = locate_alive(&world, first).arch_index();
        let second_table = locate_alive(&world, second).arch_index();
        assert_eq!(first_table, second_table, "table of each entity");
        assert_eq!(world.archetypes.len(), 2, "number of tables");
    }

    /// Checks that a bundle with a type of size zero stores every component that it contains
    #[test]
    fn add_components_with_zero_size_type_stores_every_component() {
        let mut world = World::new();
        let entity = world.spawn_empty();

        let added = world.add_components(entity, (Marker, Health(3)));

        assert_eq!(added, Ok(()), "answer for a living entity");
        assert_eq!(world.get::<Marker>(entity), Some(&Marker), "marker");
        assert_eq!(world.get::<Health>(entity), Some(&Health(3)), "health");
    }

    /// Checks the largest bundle, of thirty-two fields, which moves the entity to one table with a
    /// column for each field
    #[test]
    fn add_components_with_32_fields_stores_every_component() {
        let mut world = World::new();
        let entity = world.spawn_empty();

        let added = world.add_components(entity, build_bundle_32()).is_ok();

        let location = locate_alive(&world, entity);
        let mask = world.archetypes[location.arch_index()].mask();
        let positions: Vec<usize> = (0..32).collect();
        assert!(added, "answer for a living entity");
        assert_eq!(
            mask,
            ComponentMask::from_positions(&positions),
            "mask of the entity's table"
        );
        assert_eq!(
            world.get::<Numbered<0>>(entity),
            Some(&Numbered(0)),
            "first component"
        );
        assert_eq!(
            world.get::<Numbered<31>>(entity),
            Some(&Numbered(31)),
            "last component"
        );
    }

    /// Checks that `add_components` panics on a bundle that lists one type twice
    #[test]
    #[should_panic]
    fn add_components_with_same_type_twice_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty();

        world
            .add_components(entity, (Health(1), Health(2)))
            .expect("the entity should be alive");
    }

    /// Checks that `reserve` creates an empty table for new types, and gives each type a bit
    #[test]
    fn reserve_with_new_types_creates_empty_table_with_their_bits() {
        let mut world = World::new();

        world.reserve::<(Position, Health)>(4);

        let position_bit = world.arch_masks[&TypeId::of::<Position>()];
        let health_bit = world.arch_masks[&TypeId::of::<Health>()];
        let table = &world.archetypes[1];
        let table_index = world.indices.get(&(position_bit | health_bit));
        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            [position_bit, health_bit],
            [ComponentMask::single(0), ComponentMask::single(1)],
            "bits in order of the bundle"
        );
        assert_eq!(table.mask(), position_bit | health_bit, "mask of the table");
        assert_eq!(table_index, Some(&1), "table under its mask");
        assert_eq!(table.len(), 0, "rows in the table");
        assert!(world.slots.capacity() >= 4, "room in the list of slots");
    }

    /// Checks that `reserve` leaves the list of slots at its capacity, where the freed slots can
    /// take every entity that it makes room for
    #[test]
    fn reserve_with_enough_freed_slots_keeps_slot_capacity() {
        let mut world = World::new();
        // Spawns entities until the list of slots is full, meaning that any more room in the list
        // would need a larger block
        let mut entities = vec![world.spawn_empty()];
        while world.slots.len() < world.slots.capacity() {
            entities.push(world.spawn_empty());
        }
        for &entity in &entities {
            world.despawn(entity);
        }
        let capacity = world.slots.capacity();

        world.reserve::<(Health,)>(entities.len());

        assert_eq!(
            world.slots.capacity(),
            capacity,
            "room in the list of slots"
        );
    }

    /// Checks that entities spawned after `reserve` reach the reserved table, with their own values
    #[test]
    fn reserve_with_varying_counts_keeps_later_entities_in_reserved_table() {
        let cases = [0_usize, 1, 5];

        for count in cases {
            let mut world = World::new();
            world.reserve::<(Position, Health)>(count);

            let entities: Vec<Entity> = (0..count)
                .map(|value| {
                    let entity = world.spawn_empty();
                    world
                        .add_components(entity, (Position(value as f32), Health(value as u32)))
                        .expect("the entity should be alive");
                    entity
                })
                .collect();

            assert_eq!(world.archetypes.len(), 2, "number of tables for {count}");
            assert_eq!(world.archetypes[1].len(), count, "rows for {count}");
            for (value, entity) in entities.into_iter().enumerate() {
                let table = locate_alive(&world, entity).arch_index();
                let health = world.get::<Health>(entity);
                assert_eq!(table, 1, "table of entity {value} for {count}");
                assert_eq!(
                    health,
                    Some(&Health(value as u32)),
                    "health of entity {value} for {count}"
                );
            }
        }
    }

    /// Checks that `reserve` finds the table that already stores its types, in any order
    #[test]
    fn reserve_with_existing_table_reuses_table() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, (Position(1.0), Health(1)))
            .expect("the entity should be alive");

        world.reserve::<(Health, Position)>(3);

        assert_eq!(world.archetypes.len(), 2, "number of tables");
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(1)),
            "stored component"
        );
    }

    /// Checks that `reserve` panics on a bundle type that lists one type twice
    #[test]
    #[should_panic]
    fn reserve_with_same_type_twice_panics() {
        let mut world = World::new();

        world.reserve::<(Health, Health)>(1);
    }

    /// Checks that `reserve` panics where the room asked for would exceed `isize::MAX` bytes
    #[test]
    #[should_panic]
    fn reserve_with_usize_max_additional_panics() {
        let mut world = World::new();

        world.reserve::<(Health,)>(usize::MAX);
    }

    /// Checks that a type seen before keeps its bit, even once every bit is taken
    #[test]
    fn find_or_assign_bit_with_full_world_and_known_type_returns_its_bit() {
        let mut world = World::new();
        let type_ids = build_64_type_ids();
        for type_id in type_ids {
            world.find_or_assign_bit(type_id);
        }

        let last = world.find_or_assign_bit(type_ids[63]);

        assert_eq!(last, ComponentMask::single(63), "bit of the last type");
        assert_eq!(world.arch_masks.len(), 64, "component types");
    }

    /// Checks that `find_or_assign_bit` panics on a 65th type, since a mask has room for 64 bits
    #[test]
    #[should_panic]
    fn find_or_assign_bit_with_65th_type_panics() {
        let mut world = World::new();
        for type_id in build_64_type_ids() {
            world.find_or_assign_bit(type_id);
        }

        world.find_or_assign_bit(TypeId::of::<Health>());
    }

    /// Checks that `get` returns `None` for a type that the world has never met
    #[test]
    fn get_with_type_never_seen_returns_none() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        let found = world.get::<Velocity>(entity);

        assert_eq!(found, None);
    }

    /// Checks that a handle at another version of a living entity's slot reaches nothing
    #[test]
    fn get_with_other_version_of_living_slot_returns_none() {
        let cases = [
            ("later version", Entity::new(0, 1)),
            ("largest version", Entity::new(0, u32::MAX)),
        ];
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        let answers = cases.map(|(label, handle)| (label, world.get::<Health>(handle)));

        for (label, found) in answers {
            assert_eq!(found, None, "component for {label}");
        }
    }

    /// Checks that `get_mut` returns `None` for every handle that does not refer to a living entity
    #[test]
    fn get_mut_with_handle_of_no_living_entity_returns_none() {
        let cases = [
            ("index past the last slot", Entity::new(9, 0)),
            ("later version", Entity::new(0, 1)),
            ("largest index and version", Entity::new(u32::MAX, u32::MAX)),
        ];
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        let answers =
            cases.map(|(label, handle)| (label, world.get_mut::<Health>(handle).is_none()));

        for (label, missing) in answers {
            assert!(missing, "answer for {label}");
        }
    }

    /// Checks that `get_mut` returns `None` for a type that the world has never met
    #[test]
    fn get_mut_with_type_never_seen_returns_none() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        let missing = world.get_mut::<Velocity>(entity).is_none();

        assert!(missing);
    }

    /// Checks that `query_mut` visits only the entities with every type in the query, across tables
    #[test]
    fn query_mut_with_several_tables_visits_only_entities_with_every_type() {
        let mut world = World::new();
        let alone = world.spawn_empty();
        let paired = world.spawn_empty();
        let tripled = world.spawn_empty();
        let health_only = world.spawn_empty();
        world
            .add_component(alone, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_components(paired, (Position(2.0), Health(2)))
            .expect("the entity should be alive");
        world
            .add_components(tripled, (Position(3.0), Health(3), Velocity(3.0)))
            .expect("the entity should be alive");
        world
            .add_component(health_only, Health(4))
            .expect("the entity should be alive");

        let mut found: Vec<(f32, u32)> = world
            .query_mut::<(&Position, &Health)>()
            .map(|(position, health)| (position.0, health.0))
            .collect();
        found.sort_by(|left, right| left.0.total_cmp(&right.0));

        assert_eq!(found, [(2.0, 2), (3.0, 3)]);
    }

    /// Checks that `query_mut` visits the entities in one table in row order, after a despawn
    /// reorders the rows
    #[test]
    fn query_mut_with_one_table_returns_items_in_row_order() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        for (value, entity) in entities.into_iter().enumerate() {
            world
                .add_component(entity, Health(value as u32))
                .expect("the entity should be alive");
        }
        world.despawn(entities[0]);

        let found: Vec<u32> = world
            .query_mut::<&Health>()
            .map(|health| health.0)
            .collect();

        assert_eq!(found, [3, 1, 2]);
    }

    /// Checks that a change through `query_mut` stays in the world, for every table it visits
    #[test]
    fn query_mut_with_mutable_query_keeps_changes_for_get() {
        let mut world = World::new();
        let alone = world.spawn_empty();
        let paired = world.spawn_empty();
        world
            .add_component(alone, Health(1))
            .expect("the entity should be alive");
        world
            .add_components(paired, (Position(2.0), Health(2)))
            .expect("the entity should be alive");

        for health in world.query_mut::<&mut Health>() {
            health.0 += 10;
        }

        assert_eq!(world.get::<Health>(alone), Some(&Health(11)), "first table");
        assert_eq!(
            world.get::<Health>(paired),
            Some(&Health(12)),
            "second table"
        );
    }

    /// Checks that a nested pair returns the component for each part, from the same entity
    #[test]
    fn query_mut_with_nested_pair_returns_each_part() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world
            .add_components(first, (Position(1.0), Health(1), Velocity(1.5)))
            .expect("the entity should be alive");
        world
            .add_components(second, (Position(2.0), Health(2), Velocity(2.5)))
            .expect("the entity should be alive");

        let found: Vec<(f32, u32, f32)> = world
            .query_mut::<((&Position, &mut Health), &Velocity)>()
            .map(|((position, health), velocity)| (position.0, health.0, velocity.0))
            .collect();

        assert_eq!(found, [(1.0, 1, 1.5), (2.0, 2, 2.5)]);
    }

    /// Checks that a flat tuple returns the component for each field, from the same entity, and
    /// skips an entity that lacks one of the types
    #[test]
    fn query_mut_with_flat_tuple_returns_each_field() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        let partial = world.spawn_empty();
        world
            .add_components(first, (Position(1.0), Health(1), Velocity(1.5)))
            .expect("the entity should be alive");
        world
            .add_components(second, (Position(2.0), Health(2), Velocity(2.5)))
            .expect("the entity should be alive");
        world
            .add_components(partial, (Position(3.0), Health(3)))
            .expect("the entity should be alive");

        let found: Vec<(f32, u32, f32)> = world
            .query_mut::<(&Position, &mut Health, &Velocity)>()
            .map(|(position, health, velocity)| (position.0, health.0, velocity.0))
            .collect();

        assert_eq!(found, [(1.0, 1, 1.5), (2.0, 2, 2.5)]);
    }

    /// Checks that a query that reads one type twice returns that component in both parts
    #[test]
    fn query_mut_with_two_reads_of_same_type_returns_both() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(4.0))
            .expect("the entity should be alive");

        let found: Vec<(f32, f32)> = world
            .query_mut::<(&Position, &Position)>()
            .map(|(first, second)| (first.0, second.0))
            .collect();

        assert_eq!(found, [(4.0, 4.0)]);
    }

    /// Checks that a query for a type that the world has never met returns nothing
    #[test]
    fn query_mut_with_type_never_seen_returns_nothing() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");

        let found = world.query_mut::<&Velocity>().count();

        assert_eq!(found, 0);
    }

    /// Checks that a query on a world with no entities returns nothing
    #[test]
    fn query_mut_with_empty_world_returns_nothing() {
        let mut world = World::new();

        let found = world.query_mut::<&Position>().count();

        assert_eq!(found, 0);
    }

    /// Checks that a query with a type of size zero visits each entity that has that type
    #[test]
    fn query_mut_with_zero_size_type_visits_each_entity_with_it() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        world
            .add_components(entities[0], (Marker, Health(0)))
            .expect("the entity should be alive");
        world
            .add_components(entities[1], (Marker, Health(1)))
            .expect("the entity should be alive");
        world
            .add_component(entities[2], Health(2))
            .expect("the entity should be alive");

        let found: Vec<u32> = world
            .query_mut::<(&Marker, &mut Health)>()
            .map(|(_, health)| health.0)
            .collect();

        assert_eq!(found, [0, 1]);
    }

    /// Checks that `query_mut` panics on a query that changes a type and also reads it
    #[test]
    #[should_panic]
    fn query_mut_with_write_and_read_of_same_type_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");

        let _items = world.query_mut::<(&mut Position, &Position)>();
    }

    /// Checks that `query_archetype_mut` returns one item for each matching table, with its whole
    /// column
    #[test]
    fn query_archetype_mut_with_several_matching_tables_returns_one_column_per_table() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        world
            .add_component(entities[0], Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(entities[1], Position(2.0))
            .expect("the entity should be alive");
        world
            .add_components(entities[2], (Position(3.0), Health(3)))
            .expect("the entity should be alive");
        world
            .add_component(entities[3], Health(4))
            .expect("the entity should be alive");

        let mut lengths: Vec<usize> = world
            .query_archetype_mut::<&Position>()
            .map(|positions| positions.len())
            .collect();
        lengths.sort_unstable();

        assert_eq!(lengths, [1, 2]);
    }

    /// Checks that `query_archetype_mut` skips a matching table once its last entity has left
    #[test]
    fn query_archetype_mut_with_emptied_table_skips_it() {
        let mut world = World::new();
        let leaver = world.spawn_empty();
        let stayer = world.spawn_empty();
        world
            .add_component(leaver, Position(1.0))
            .expect("the entity should be alive");
        world.remove_component::<Position>(leaver);
        world
            .add_components(stayer, (Position(2.0), Health(2)))
            .expect("the entity should be alive");

        let columns: Vec<Vec<f32>> = world
            .query_archetype_mut::<&Position>()
            .map(|positions| positions.iter().map(|position| position.0).collect())
            .collect();

        assert_eq!(columns, [[2.0]]);
    }

    /// Checks that a change to a column from `query_archetype_mut` stays in the world
    #[test]
    fn query_archetype_mut_with_mutable_column_keeps_changes_for_get() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world
            .add_components(first, (Position(1.0), Health(10)))
            .expect("the entity should be alive");
        world
            .add_components(second, (Position(2.0), Health(20)))
            .expect("the entity should be alive");

        for (positions, healths) in world.query_archetype_mut::<(&mut Position, &Health)>() {
            for (position, health) in positions.iter_mut().zip(healths) {
                position.0 += health.0 as f32;
            }
        }

        assert_eq!(
            world.get::<Position>(first),
            Some(&Position(11.0)),
            "first entity"
        );
        assert_eq!(
            world.get::<Position>(second),
            Some(&Position(22.0)),
            "second entity"
        );
    }

    /// Checks that `query_archetype_mut` panics on a nested query that changes a type it also reads
    #[test]
    #[should_panic]
    fn query_archetype_mut_with_nested_write_of_read_type_panics() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, (Position(1.0), Health(1)))
            .expect("the entity should be alive");

        let _columns = world.query_archetype_mut::<((&Position, &Health), &mut Position)>();
    }

    /// Checks that `&mut` items collected from `query_mut` across two tables can all be held at
    /// once, and that a change through each one reaches its own entity
    #[test]
    fn query_mut_with_items_collected_together_changes_each_entity() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        world
            .add_component(entities[0], Health(1))
            .expect("the entity should be alive");
        world
            .add_component(entities[1], Health(2))
            .expect("the entity should be alive");
        world
            .add_components(entities[2], (Position(3.0), Health(3)))
            .expect("the entity should be alive");
        world
            .add_components(entities[3], (Position(4.0), Health(4)))
            .expect("the entity should be alive");

        let mut healths: Vec<&mut Health> = world.query_mut::<&mut Health>().collect();
        for health in &mut healths {
            health.0 += 10;
        }

        let expected = [11, 12, 13, 14];
        for (index, (entity, value)) in entities.into_iter().zip(expected).enumerate() {
            let found = world.get::<Health>(entity);
            assert_eq!(found, Some(&Health(value)), "health of entity {index}");
        }
    }

    /// Checks that `query_mut` finds a component type at bit 63, the last bit that a mask has
    #[test]
    fn query_mut_with_component_at_bit_63_visits_entity() {
        let mut world = World::new();
        for type_id in &build_64_type_ids()[..63] {
            world.find_or_assign_bit(*type_id);
        }
        let entity = world.spawn_empty();
        world
            .add_component(entity, Health(1))
            .expect("the entity should be alive");

        let found: Vec<u32> = world
            .query_mut::<&mut Health>()
            .map(|health| {
                health.0 += 10;
                health.0
            })
            .collect();

        let health_bit = world.arch_masks.get(&TypeId::of::<Health>()).copied();
        assert_eq!(
            health_bit,
            Some(ComponentMask::single(63)),
            "bit for `Health`"
        );
        assert_eq!(found, [11], "visited components");
        assert_eq!(
            world.get::<Health>(entity),
            Some(&Health(11)),
            "stored component"
        );
    }

    /// Checks the largest query, of thirty-two fields, which reads thirty-one types and changes the
    /// last
    #[test]
    fn query_mut_with_32_fields_returns_every_field() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_components(entity, build_bundle_32())
            .unwrap_or_else(|_| panic!("the entity should be alive"));

        let found: Vec<(usize, usize, usize)> = world
            .query_mut::<Query32>()
            .map(|item| {
                item.31.0 += 100;
                (item.0.0, item.30.0, item.31.0)
            })
            .collect();

        assert_eq!(found, [(0, 30, 131)], "first, next to last and last fields");
        assert_eq!(
            world.get::<Numbered<31>>(entity),
            Some(&Numbered(131)),
            "changed component"
        );
    }

    /// Checks that `query` visits only the entities with every type in the query, across tables
    #[test]
    fn query_with_several_tables_visits_only_entities_with_every_type() {
        let mut world = World::new();
        let alone = world.spawn_empty();
        let paired = world.spawn_empty();
        let tripled = world.spawn_empty();
        let health_only = world.spawn_empty();
        world
            .add_component(alone, Position(1.0))
            .expect("the entity should be alive");
        world
            .add_components(paired, (Position(2.0), Health(2)))
            .expect("the entity should be alive");
        world
            .add_components(tripled, (Position(3.0), Health(3), Velocity(3.0)))
            .expect("the entity should be alive");
        world
            .add_component(health_only, Health(4))
            .expect("the entity should be alive");

        let mut found: Vec<(f32, u32)> = world
            .query::<(&Position, &Health)>()
            .map(|(position, health)| (position.0, health.0))
            .collect();
        found.sort_by(|left, right| left.0.total_cmp(&right.0));

        assert_eq!(found, [(2.0, 2), (3.0, 3)]);
    }

    /// Checks that a nested pair returns the component for each part, from the same entity
    #[test]
    fn query_with_nested_pair_returns_each_part() {
        let mut world = World::new();
        let first = world.spawn_empty();
        let second = world.spawn_empty();
        world
            .add_components(first, (Position(1.0), Health(1), Velocity(1.5)))
            .expect("the entity should be alive");
        world
            .add_components(second, (Position(2.0), Health(2), Velocity(2.5)))
            .expect("the entity should be alive");

        let found: Vec<(f32, u32, f32)> = world
            .query::<((&Position, &Health), &Velocity)>()
            .map(|((position, health), velocity)| (position.0, health.0, velocity.0))
            .collect();

        assert_eq!(found, [(1.0, 1, 1.5), (2.0, 2, 2.5)]);
    }

    /// Checks that a query that reads one type twice returns that component in both parts
    #[test]
    fn query_with_two_reads_of_same_type_returns_both() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(4.0))
            .expect("the entity should be alive");

        let found: Vec<(f32, f32)> = world
            .query::<(&Position, &Position)>()
            .map(|(first, second)| (first.0, second.0))
            .collect();

        assert_eq!(found, [(4.0, 4.0)]);
    }

    /// Checks that a query for a type that the world has never met returns nothing
    #[test]
    fn query_with_type_never_seen_returns_nothing() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");

        let found = world.query::<&Velocity>().count();

        assert_eq!(found, 0);
    }

    /// Checks that a query on a world with no entities returns nothing
    #[test]
    fn query_with_empty_world_returns_nothing() {
        let world = World::new();

        let found = world.query::<&Position>().count();

        assert_eq!(found, 0);
    }

    /// Checks that `query` skips a matching table once its last entity has left
    #[test]
    fn query_with_emptied_table_skips_it() {
        let mut world = World::new();
        let leaver = world.spawn_empty();
        let stayer = world.spawn_empty();
        world
            .add_component(leaver, Position(1.0))
            .expect("the entity should be alive");
        world.remove_component::<Position>(leaver);
        world
            .add_components(stayer, (Position(2.0), Health(2)))
            .expect("the entity should be alive");

        let found: Vec<f32> = world
            .query::<&Position>()
            .map(|position| position.0)
            .collect();

        assert_eq!(found, [2.0]);
    }

    /// Checks that `query_archetype` returns one item for each matching table, with its whole column
    #[test]
    fn query_archetype_with_several_matching_tables_returns_one_column_per_table() {
        let mut world = World::new();
        let entities = [
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
            world.spawn_empty(),
        ];
        world
            .add_component(entities[0], Position(1.0))
            .expect("the entity should be alive");
        world
            .add_component(entities[1], Position(2.0))
            .expect("the entity should be alive");
        world
            .add_components(entities[2], (Position(3.0), Health(3)))
            .expect("the entity should be alive");
        world
            .add_component(entities[3], Health(4))
            .expect("the entity should be alive");

        let mut lengths: Vec<usize> = world
            .query_archetype::<&Position>()
            .map(|positions| positions.len())
            .collect();
        lengths.sort_unstable();

        assert_eq!(lengths, [1, 2]);
    }

    /// Checks that `query_archetype` skips a matching table once its last entity has left
    #[test]
    fn query_archetype_with_emptied_table_skips_it() {
        let mut world = World::new();
        let leaver = world.spawn_empty();
        let stayer = world.spawn_empty();
        world
            .add_component(leaver, Position(1.0))
            .expect("the entity should be alive");
        world.remove_component::<Position>(leaver);
        world
            .add_components(stayer, (Position(2.0), Health(2)))
            .expect("the entity should be alive");

        let columns: Vec<Vec<f32>> = world
            .query_archetype::<&Position>()
            .map(|positions| positions.iter().map(|position| position.0).collect())
            .collect();

        assert_eq!(columns, [[2.0]]);
    }

    /// Checks that `query_archetype` for a type that the world has never met returns nothing
    #[test]
    fn query_archetype_with_type_never_seen_returns_nothing() {
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(1.0))
            .expect("the entity should be alive");

        let found = world.query_archetype::<&Velocity>().count();

        assert_eq!(found, 0);
    }

    /// Checks that a location keeps the table and the row that it receives, up to `u32::MAX`
    #[test]
    fn location_new_with_values_up_to_u32_max_returns_same_values() {
        let max = u32::MAX as usize;
        let cases = [(0, 0), (3, 7), (max, max)];

        for (arch_index, row) in cases {
            let location = Location::new(arch_index, row);

            let found = (location.arch_index(), location.row());
            assert_eq!(found, (arch_index, row), "location ({arch_index}, {row})");
        }
    }

    /// Checks that `Location::new` panics on a table position past `u32::MAX`
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic]
    fn location_new_with_arch_index_past_u32_max_panics() {
        Location::new(u32::MAX as usize + 1, 0);
    }

    /// Checks that `Location::new` panics on a row past `u32::MAX`
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic]
    fn location_new_with_row_past_u32_max_panics() {
        Location::new(0, u32::MAX as usize + 1);
    }

    /// Checks that `set_row` changes the row and keeps the table
    #[test]
    fn set_row_with_new_row_keeps_table() {
        let mut location = Location::new(2, 5);

        location.set_row(9);

        let found = (location.arch_index(), location.row());
        assert_eq!(found, (2, 9));
    }

    /// Checks that `set_row` panics on a row past `u32::MAX`
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic]
    fn set_row_with_row_past_u32_max_panics() {
        let mut location = Location::new(2, 5);

        location.set_row(u32::MAX as usize + 1);
    }

    /// Checks that the slots, the tables and `indices` agree after every step of a sequence that
    /// spawns, adds, queries, removes, despawns, reuses a slot and retires one
    #[test]
    fn world_with_mixed_operation_sequence_keeps_slots_and_rows_consistent() {
        let mut world = World::new();

        let first = world.spawn_empty();
        check_invariants(&world);
        let second = world.spawn_empty();
        check_invariants(&world);
        let third = world.spawn_empty();
        check_invariants(&world);
        world
            .add_component(first, Position(1.0))
            .expect("the entity should be alive");
        check_invariants(&world);
        world
            .add_components(second, (Position(2.0), Health(20)))
            .expect("the entity should be alive");
        check_invariants(&world);
        world
            .add_component(third, Health(30))
            .expect("the entity should be alive");
        check_invariants(&world);
        world
            .add_component(first, Health(10))
            .expect("the entity should be alive");
        check_invariants(&world);
        let pairs = world.query::<(&Position, &Health)>().count();
        check_invariants(&world);
        let removed = world.remove_component::<Position>(second);
        check_invariants(&world);
        world
            .add_components(third, (Velocity(3.0), Marker))
            .expect("the entity should be alive");
        check_invariants(&world);
        world.despawn(first);
        check_invariants(&world);
        let reborn = world.spawn_empty();
        check_invariants(&world);
        world
            .add_component(reborn, Velocity(4.0))
            .expect("the entity should be alive");
        check_invariants(&world);

        // Frees a new slot, then sets it one version short of `u32::MAX`
        let doomed = world.spawn_empty();
        world.despawn(doomed);
        world.slots[doomed.index()].version = u32::MAX - 1;
        check_invariants(&world);
        // Fills the slot with the last entity that it can serve, then despawns that entity, which
        // retires the slot
        let last = world.spawn_empty();
        check_invariants(&world);
        world
            .add_component(last, Health(99))
            .expect("the entity should be alive");
        check_invariants(&world);
        world.despawn(last);
        check_invariants(&world);

        let after = world.spawn_empty();
        check_invariants(&world);
        world.remove_component::<Health>(third);
        check_invariants(&world);
        world.despawn(second);
        check_invariants(&world);

        assert_eq!(pairs, 2, "entities with a position and health");
        assert_eq!(removed, Some(Position(2.0)), "removed component");
        assert_eq!(
            reborn,
            Entity::new(0, 1),
            "handle that reuses the first slot"
        );
        assert_eq!(
            last.version(),
            u32::MAX - 1,
            "version of the last entity in its slot"
        );
        assert_eq!(after, Entity::new(4, 0), "handle after the retired slot");
        assert_eq!(world.free_slots, [1], "free slots");
        assert_eq!(
            world.get::<Health>(first),
            None,
            "component of the despawned entity"
        );
        assert_eq!(
            world.get::<Velocity>(reborn),
            Some(&Velocity(4.0)),
            "component of the entity in the reused slot"
        );
        assert_eq!(
            world.get::<Velocity>(third),
            Some(&Velocity(3.0)),
            "kept component of the third entity"
        );
        assert_eq!(
            world.get::<Health>(third),
            None,
            "removed component of the third entity"
        );
        assert_eq!(
            world.get::<Marker>(third),
            Some(&Marker),
            "marker of the third entity"
        );
    }

    /// Checks that thousands of random operations give the same answers as a `HashMap` that
    /// stores each living entity's components, and leave the slots and tables consistent
    #[test]
    fn world_with_random_operations_matches_hashmap_model() {
        /// Stores the components that the model expects one entity to have.
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        struct Expected {
            position: Option<f32>,
            health: Option<u32>,
            velocity: Option<f32>,
            marker: bool,
        }

        /// Generates numbers with a 64-bit xorshift, which repeats the same run for one seed.
        struct Xorshift(u64);

        impl Xorshift {
            /// Advances the state, and returns a number in `0..bound`.
            fn next_below(&mut self, bound: usize) -> usize {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                (self.0 % bound as u64) as usize
            }
        }

        /// Reads the components that `entity` has in `world`, in the form that the model stores.
        fn read_actual(world: &World, entity: Entity) -> Expected {
            Expected {
                position: world.get::<Position>(entity).map(|position| position.0),
                health: world.get::<Health>(entity).map(|health| health.0),
                velocity: world.get::<Velocity>(entity).map(|velocity| velocity.0),
                marker: world.get::<Marker>(entity).is_some(),
            }
        }

        /// Panics where any handle in `handles` reaches different components in `world` than the
        /// model expects. A handle missing from the model expects none.
        fn check_matches_model(
            world: &World,
            handles: &[Entity],
            model: &HashMap<Entity, Expected>,
            step: usize,
        ) {
            for &entity in handles {
                let expected = model.get(&entity).copied().unwrap_or_default();
                let alive = world.locate(entity).is_some();
                assert_eq!(
                    alive,
                    model.contains_key(&entity),
                    "{entity:?} at step {step}"
                );
                assert_eq!(
                    read_actual(world, entity),
                    expected,
                    "{entity:?} at step {step}"
                );
            }
        }

        let mut world = World::new();
        let mut model: HashMap<Entity, Expected> = HashMap::new();
        let mut handles: Vec<Entity> = Vec::new();
        let mut rng = Xorshift(0x2545_F491_4F6C_DD1D);

        for step in 0..5000 {
            // Spawns where no handle exists yet, and otherwise picks one of the twelve operations
            let operation = if handles.is_empty() {
                0
            } else {
                rng.next_below(12)
            };
            let entity = if handles.is_empty() {
                Entity::new(0, 0)
            } else {
                handles[rng.next_below(handles.len())]
            };
            let value = rng.next_below(1000);

            let entity = match operation {
                0 | 1 => {
                    let spawned = world.spawn_empty();
                    assert!(!handles.contains(&spawned), "new handle at step {step}");
                    model.insert(spawned, Expected::default());
                    handles.push(spawned);
                    spawned
                }
                2 => {
                    let answer = world.despawn(entity);
                    let expected = model.remove(&entity).is_some();
                    assert_eq!(answer, expected, "despawn at step {step}");
                    entity
                }
                3 => {
                    let position = value as f32;
                    let answer = world.add_component(entity, Position(position));
                    let expected = model
                        .get_mut(&entity)
                        .map(|components| components.position = Some(position))
                        .ok_or(Position(position));
                    assert_eq!(
                        answer, expected,
                        "add_component of a position at step {step}"
                    );
                    entity
                }
                4 => {
                    let health = value as u32;
                    let answer = world.add_component(entity, Health(health));
                    let expected = model
                        .get_mut(&entity)
                        .map(|components| components.health = Some(health))
                        .ok_or(Health(health));
                    assert_eq!(answer, expected, "add_component of health at step {step}");
                    entity
                }
                5 => {
                    let velocity = value as f32;
                    let answer = world.add_component(entity, Velocity(velocity));
                    let expected = model
                        .get_mut(&entity)
                        .map(|components| components.velocity = Some(velocity))
                        .ok_or(Velocity(velocity));
                    assert_eq!(
                        answer, expected,
                        "add_component of a velocity at step {step}"
                    );
                    entity
                }
                6 => {
                    let (health, velocity) = (value as u32, value as f32);
                    let answer = world.add_components(entity, (Health(health), Velocity(velocity)));
                    let expected = model
                        .get_mut(&entity)
                        .map(|components| {
                            components.health = Some(health);
                            components.velocity = Some(velocity);
                        })
                        .ok_or((Health(health), Velocity(velocity)));
                    assert_eq!(answer, expected, "add_components at step {step}");
                    entity
                }
                7 => {
                    let position = value as f32;
                    let answer = world.add_components(entity, (Position(position), Marker));
                    let expected = model
                        .get_mut(&entity)
                        .map(|components| {
                            components.position = Some(position);
                            components.marker = true;
                        })
                        .ok_or((Position(position), Marker));
                    assert_eq!(
                        answer, expected,
                        "add_components with a marker at step {step}"
                    );
                    entity
                }
                8 => {
                    let answer = world.remove_component::<Position>(entity);
                    let expected = model
                        .get_mut(&entity)
                        .and_then(|components| components.position.take())
                        .map(Position);
                    assert_eq!(
                        answer, expected,
                        "remove_component of a position at step {step}"
                    );
                    entity
                }
                9 => {
                    let answer = world.remove_component::<Health>(entity);
                    let expected = model
                        .get_mut(&entity)
                        .and_then(|components| components.health.take())
                        .map(Health);
                    assert_eq!(
                        answer, expected,
                        "remove_component of health at step {step}"
                    );
                    entity
                }
                10 => {
                    let answer = world.remove_component::<Velocity>(entity);
                    let expected = model
                        .get_mut(&entity)
                        .and_then(|components| components.velocity.take())
                        .map(Velocity);
                    assert_eq!(
                        answer, expected,
                        "remove_component of a velocity at step {step}"
                    );
                    entity
                }
                _ => {
                    let answer = world.remove_component::<Marker>(entity);
                    let expected = model.get_mut(&entity).and_then(|components| {
                        let had_marker = components.marker;
                        components.marker = false;
                        had_marker.then_some(Marker)
                    });
                    assert_eq!(
                        answer, expected,
                        "remove_component of a marker at step {step}"
                    );
                    entity
                }
            };

            // Reads back the entity that the operation touched, and checks the whole world at
            // regular steps
            check_matches_model(&world, &[entity], &model, step);
            if step % 250 == 0 {
                check_invariants(&world);
                check_matches_model(&world, &handles, &model, step);
            }
        }

        let healthy = model.values().filter(|expected| expected.health.is_some());
        let moving = healthy
            .clone()
            .filter(|expected| expected.velocity.is_some());
        let expected_counts = (healthy.count(), moving.count());
        let found_counts = (
            world.query::<&Health>().count(),
            world.query::<(&Health, &Velocity)>().count(),
        );
        check_invariants(&world);
        check_matches_model(&world, &handles, &model, 5000);
        assert_eq!(
            found_counts, expected_counts,
            "entities that each query visits"
        );
    }
}
