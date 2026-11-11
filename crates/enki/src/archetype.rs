//! Defines a table in column-major order that stores every entity with one exact set of component
//! types.
//!
//! An archetype is one set of component types, such as `{Position, Velocity}`. An [`Archetype`] is
//! the table for one such set, as ADR 0011 sets out. The table has a list of entities, and one
//! column for each component type. Each column is a [`ComponentVec`].
//!
//! # Rows
//!
//! A row number ties an entity to its components. The entity at row 5 in the list owns the
//! component at index 5 in every column.
//!
//! The caller adds a row with [`Archetype::push_entity`]. The method pushes the entity, then calls a
//! closure from the caller, which pushes one component onto every column. Once the closure returns,
//! the method checks that every column again has one component for each entity in the list, and
//! panics where one does not. The gap between the two steps therefore never outlasts the call.
//!
//! # Masks
//!
//! A table has a mask, which is a [`ComponentMask`] with one set bit for each component type that
//! the table stores. Each bit comes from the [`ComponentInfo`] for that type. A `ComponentInfo`
//! describes one component type to the table.
//!
//! A caller that wants a set of component types builds a mask of those types. It then passes that
//! mask to [`Archetype::has_all`]. The method answers whether the table stores every one of those
//! types, from the two masks alone.
//!
//! # Moves between tables
//!
//! An entity moves to another table when it gains or loses a component. The caller handles both
//! cases with [`Archetype::move_entity`]. Where the target lacks a column for one of the entity's
//! components, the method passes that column to a closure from the caller, which drops the
//! component or keeps it as a value. The last row in the old table then fills the empty row, which
//! keeps each column in one unbroken run.
//!
//! Where the target has a column that the old table lacks, the caller passes a closure that pushes
//! one component onto each such column, as with `push_entity`. The method checks the target once
//! the closure returns.
//!
//! The caller records a location for each entity, which is its table and its row. A move changes
//! the location of the moved entity, and of the entity that fills the empty row, where there is
//! one. The caller MUST update both. A caller that keeps a stale row can later pass an index past
//! the end of a column to `move_entity`, which then panics before anything changes.
//!
//! # Examples
//!
//! The example adds two entities to a table, then gives the first entity a `Health` component by
//! moving it to a table that also stores `Health`.
//!
//! ```ignore
//! struct Position(f32);
//! struct Health(u32);
//!
//! // Creates a table for `{Position}`, and a table for `{Position, Health}`
//! let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
//! let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
//! let mut source = Archetype::new(&[position]);
//! let mut target = Archetype::new(&[position, health]);
//!
//! // Adds two rows to the source table, with the closure pushing each entity's `Position`
//! let first = Entity::new(0, 0);
//! let second = Entity::new(1, 0);
//! for (entity, x) in [(first, 1.0), (second, 2.0)] {
//!     source.push_entity(entity, |table, _row| {
//!         let positions = table
//!             .components_of_mut(position.type_id())
//!             .expect("the table should store `Position`"); // &mut ComponentVec
//!         positions.push_typed(Position(x));
//!     });
//! }
//!
//! // Moves row 0 to the target table, gives the moved entity its new `Health`, and fills row 0
//! // with the last row
//! let filled = source.move_entity(0, &mut target, ComponentVec::swap_remove, |table, _row| {
//!     let healths = table
//!         .components_of_mut(health.type_id())
//!         .expect("the table should store `Health`");
//!     healths.push_typed(Health(100));
//! });
//!
//! // Checks that `second` now occupies row 0 in the source table, and that each table has one row
//! assert_eq!(filled, Some(second));
//! assert_eq!(source.len(), 1);
//! assert_eq!(target.len(), 1);
//! ```

use crate::Entity;
use crate::component::{ComponentInfo, ComponentVec};
use crate::mask::ComponentMask;
use std::any::TypeId;
use std::{mem, process};

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Stores every entity with one set of component types, as rows across one column per type.
///
/// A caller reaches a column by [`TypeId`], through [`components_of`](Self::components_of) or
/// [`components_of_mut`](Self::components_of_mut). The [module example](self#examples) shows how a
/// caller adds a row and moves one to another table.
pub struct Archetype {
    /// Stores one column for each component type, sorted by `TypeId`.
    ///
    /// The [module example](self#examples) fills the `Position` column and the `Health` column.
    components: Vec<ComponentVec>,
    /// Stores the `TypeId` of each column, in the same order as `components`.
    type_ids: Vec<TypeId>,
    /// Stores the entity that owns each row, in row order.
    ///
    /// The [module example](self#examples) adds two entities to this list.
    entities: Vec<Entity>,
    /// Stores one set bit for each component type in `components`.
    ///
    /// The [module example](self#examples) builds a table whose mask sets bits 0 and 1.
    mask: ComponentMask,
}

impl Archetype {
    // ========================================================================================== \\
    //                                        Constructors                                        \\
    // ========================================================================================== \\

    /// Returns an empty table with one column for each entry in `infos`. The caller can list the
    /// entries in any order.
    ///
    /// The mask combines the bit from each entry. An empty `infos` gives a table with no columns and
    /// the mask [`ComponentMask::EMPTY`].
    ///
    /// The caller MUST pass each component type once, with a bit that follows the rules in
    /// [`ComponentInfo::of`]. A type passed twice with two different bits gets two columns. A search
    /// for that type returns one column every time, and which one is unspecified. Every push then
    /// reaches the column that the search returns, and the other column stays empty. The first
    /// call to [`push_entity`](Self::push_entity) then panics, since that column lacks a component
    /// for the new row.
    ///
    /// # Panics
    ///
    /// Panics in a debug build where two entries in `infos` share a bit. A release build skips that
    /// check, since the world builds every list that it passes here with a different bit for each
    /// type.
    ///
    /// # Examples
    ///
    /// The [module example](self#examples) builds a table for `{Position}` and a table for
    /// `{Position, Health}`.
    pub(crate) fn new(infos: &[ComponentInfo]) -> Self {
        // Creates one empty column for each entry in `infos`
        let mut components: Vec<ComponentVec> =
            infos.iter().map(|info| ComponentVec::new(*info)).collect();
        // Sorts the columns by `TypeId`, which the binary search in `find_index_of` requires
        components.sort_by_key(|components| components.info().type_id());
        let type_ids = components
            .iter()
            .map(|components| components.info().type_id())
            .collect();
        let mut mask = ComponentMask::EMPTY;
        for info in infos {
            // Stops two types from sharing a bit, which would make queries treat them as one type
            debug_assert!(
                !mask.intersects(info.bit()),
                "two component types in one table share a mask bit"
            );
            mask |= info.bit();
        }

        Self {
            components,
            type_ids,
            entities: Vec::new(),
            mask,
        }
    }

    // ========================================================================================== \\
    //                                          Getters                                           \\
    // ========================================================================================== \\

    /// Returns the number of entities in the table.
    ///
    /// The [module example](self#examples) checks that each table has one entity after a move.
    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns `true` where the table has no entities, and `false` where it has at least one.
    ///
    /// A new table returns `true`. Once [`push_entity`](Self::push_entity) adds an entity, it
    /// returns `false`.
    pub(crate) fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Returns the [`ComponentInfo`] for each component type in the table, sorted by [`TypeId`].
    ///
    /// Each call builds a new `Vec`, which the caller then owns.
    ///
    /// # Examples
    ///
    /// For a table built from `Position` and `Health`, `infos` returns both entries sorted by
    /// `TypeId`, whatever order the caller passed them in. The test
    /// `infos_with_any_entry_order_returns_each_type_and_bit_sorted_by_type_id` checks both orders.
    pub(crate) fn infos(&self) -> Vec<ComponentInfo> {
        self.components.iter().map(ComponentVec::info).collect()
    }

    /// Returns the mask of the table, with one set bit for each component type that it stores.
    ///
    /// # Examples
    ///
    /// For a table built from `Position` at bit 0 and `Health` at bit 1, `mask` returns the mask
    /// with bits 0 and 1 set. The test
    /// `new_with_varying_infos_returns_empty_table_with_combined_mask` checks the mask for several
    /// sets of types.
    pub(crate) fn mask(&self) -> ComponentMask {
        self.mask
    }

    /// Returns the entities in row order.
    ///
    /// The entity at index `i` in the returned slice owns the component at index `i` in every
    /// column. The slice borrows the table, meaning the compiler rejects any change to the table
    /// while the slice is in use.
    ///
    /// # Examples
    ///
    /// After pushes of `first` and then `second`, `entities` returns `[first, second]`. The test
    /// `entities_with_varying_row_counts_returns_entities_in_row_order` checks several row counts.
    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    // ========================================================================================== \\
    //                                        API Methods                                         \\
    // ========================================================================================== \\

    /// Returns `true` where the table stores every component type in `query_mask`, and `false` where
    /// it lacks at least one.
    ///
    /// A `query_mask` of [`ComponentMask::EMPTY`] does not ask for any component types, and the
    /// method then returns `true` for every table.
    ///
    /// # Examples
    ///
    /// For a table with bits 0, 1 and 3, `has_all` returns `true` for the mask with bits 0 and 1,
    /// `true` for `ComponentMask::EMPTY`, and `false` for bit 2. The test
    /// `has_all_with_varying_query_masks_returns_expected_answer` checks more masks.
    pub(crate) fn has_all(&self, query_mask: ComponentMask) -> bool {
        self.mask.contains_all(query_mask)
    }

    /// Adds `entity` to the end of the entity list, then calls `fill` to push its components.
    ///
    /// The entity takes the row `len() - 1`, measured after the call. The method passes the table
    /// and that row to `fill`, which MUST push one component onto every column. A table with no
    /// columns takes a `fill` that does nothing, written `|_, _| {}`.
    ///
    /// The method calls `fill` once. Its bound, `FnOnce`, therefore lets the closure move the
    /// components that it captures into the table.
    ///
    /// # Panics
    ///
    /// Panics where any column lacks a component for the new row once `fill` returns. Also panics
    /// where the list is full and a larger block would exceed `isize::MAX` bytes. The program
    /// aborts where the allocator cannot supply the larger block.
    ///
    /// # Examples
    ///
    /// The [module example](self#examples) adds two entities, with a `fill` that pushes each
    /// entity's `Position`.
    pub(crate) fn push_entity(&mut self, entity: Entity, fill: impl FnOnce(&mut Archetype, usize)) {
        self.entities.push(entity);
        let row = self.entities.len() - 1;
        fill(self, row);

        // Panics where `fill` left a column without a component for the new row
        assert!(
            self.has_one_component_per_entity(),
            "the fill must push one component onto every column"
        );
    }

    /// Makes room for at least `additional` more entities, in every column and in the entity list.
    ///
    /// The next `additional` entities that the caller pushes then fit without a larger block of
    /// memory. Where a column moves to a larger block, every pointer into its old block stops being
    /// valid.
    ///
    /// # Panics
    ///
    /// Panics where the required capacity overflows a `usize`, or where a larger block would exceed
    /// `isize::MAX` bytes. The program aborts where the allocator cannot supply the block.
    ///
    /// # Examples
    ///
    /// The test `reserve_with_varying_additional_rows_fits_rows_without_moving_blocks` makes room
    /// for a number of entities, pushes that many, and checks that neither the entity list nor any
    /// column moved to a larger block.
    pub(crate) fn reserve(&mut self, additional: usize) {
        for components in &mut self.components {
            components.reserve(additional);
        }
        self.entities.reserve(additional);
    }

    /// Returns the column for `type_id`, or `None` where the table has no such column.
    ///
    /// The returned reference borrows the table, meaning the compiler rejects any change to the
    /// table while the reference is in use.
    ///
    /// # Examples
    ///
    /// The test `components_of_with_stored_and_absent_types_finds_only_stored_columns` asks a table
    /// for a type that it stores, which returns the column, and for one that it lacks, which
    /// returns `None`.
    pub(crate) fn components_of(&self, type_id: TypeId) -> Option<&ComponentVec> {
        let index = self.find_index_of(type_id)?;
        Some(&self.components[index])
    }

    /// Returns the column for `type_id` so that the caller can change it, or `None` where the table
    /// has no such column.
    ///
    /// The returned reference borrows the table exclusively, meaning the compiler rejects any other
    /// use of the table while the reference is in use.
    ///
    /// A caller that pushes onto the column, moves a component out of it or replaces the column MUST
    /// then leave every column with one component for each entity in the entity list, as the Rows
    /// section in the module documentation sets out. A replacement column MUST also store the same
    /// component type, since the search depends on the sorted order.
    ///
    /// # Examples
    ///
    /// The [module example](self#examples) pushes a `Position` and a `Health` through the columns
    /// that this method returns.
    pub(crate) fn components_of_mut(&mut self, type_id: TypeId) -> Option<&mut ComponentVec> {
        let index = self.find_index_of(type_id)?;
        Some(&mut self.components[index])
    }

    /// Returns the columns for the `N` types in `type_ids`, in the same order, for the caller to
    /// change all of them at once.
    ///
    /// Returns `None` where the table has no column for one of the types, or where `type_ids` lists
    /// the same type twice. Two references to one column would let the caller change it through
    /// both, which the borrow rules forbid.
    ///
    /// `N` is a const generic, which is a number that the caller fixes at compile time, in the way
    /// that a type parameter fixes a type. A call with an array of two `TypeId` values returns an
    /// array of two columns.
    ///
    /// Each returned reference borrows the table exclusively, meaning the compiler rejects any
    /// other use of the table while any of them is in use. The rules in
    /// [`components_of_mut`](Self::components_of_mut) apply to each column.
    ///
    /// # Examples
    ///
    /// The test `get_disjoint_components_mut_with_stored_types_returns_columns_in_requested_order`
    /// borrows two columns at once, and receives them in the order of the requested types.
    pub(crate) fn get_disjoint_components_mut<const N: usize>(
        &mut self,
        type_ids: [TypeId; N],
    ) -> Option<[&mut ComponentVec; N]> {
        // Finds the position of each requested column, and returns `None` where one is missing
        let mut indices = [0; N];
        for (index, type_id) in indices.iter_mut().zip(type_ids) {
            *index = self.find_index_of(type_id)?;
        }
        // Borrows every requested column at once, and returns `None` where two positions are equal
        self.components.get_disjoint_mut(indices).ok()
    }

    /// Removes the entity at `row` from the table, and drops its components.
    ///
    /// The last row in this table then fills `row`, in the entity list and in every column.
    ///
    /// The method returns the entity that now occupies `row`, or `None` where `row` was the last
    /// row, since every other entity then keeps its row. The caller MUST update the location that it
    /// records for the returned entity, as the module documentation sets out.
    ///
    /// # Panics
    ///
    /// Panics where `row` is not below the length of the entity list, before anything changes.
    ///
    /// # Aborts
    ///
    /// The program aborts where any panic occurs once the removal has started, such as a panic in
    /// the `Drop` implementation of a component. The panic message prints first. The method removes
    /// one column at a time. Such a panic would therefore leave the columns already visited one
    /// component shorter than the other columns and the entity list. Ending the program stops any
    /// later call from using the table in that state.
    ///
    /// # Examples
    ///
    /// The test `remove_entity_with_varying_rows_fills_gap_with_last_row` removes rows from a table
    /// of several rows. Removing row 0 moves the last entity into row 0 and returns it. Removing
    /// the last row returns `None`.
    pub(crate) fn remove_entity(&mut self, row: usize) -> Option<Entity> {
        // Panics before anything changes, where `row` lies past the last entity
        assert!(
            row < self.entities.len(),
            "the row must be below the number of entities"
        );
        // Panics in a debug build, where a column and the entity list differ in length
        debug_assert!(
            self.has_one_component_per_entity(),
            "every column must have one component for each row"
        );

        // Ends the program where anything below panics, since the columns would then differ in
        // length
        let guard = AbortOnUnwind;
        // Visits each column in the table
        for components in &mut self.components {
            // Drops the entity's component, and fills `row` with the column's last component
            components.swap_remove(row);
        }
        // Removes the entity from the list in the same way, filling `row` with the last entity
        self.entities.swap_remove(row);
        // Disarms the guard, since every column and the entity list have the same length again
        mem::forget(guard);

        // Returns the entity that now occupies `row`, or `None` where `row` was the last row
        self.entities.get(row).copied()
    }

    /// Moves the entity at `row` to a new row at the end of `target`, **passing each column that
    /// `target` lacks to `take`**.
    ///
    /// The method serves both moves between tables. Where the entity gains components, `target`
    /// has every column in this table plus one or more others. Where the entity loses a component,
    /// `target` lacks that one column.
    ///
    /// The method calls `take` once for each column that `target` lacks, with that column and
    /// `row`. The closure MUST remove the component at `row` from the column, with the last
    /// component filling the gap. [`ComponentVec::swap_remove`] drops the component, and a closure
    /// that calls [`ComponentVec::swap_remove_typed`] keeps it as a value instead. A move that adds
    /// columns never calls `take`.
    ///
    /// Once every component is moved or taken, the method passes `target` and the entity's new
    /// row to `fill`. The closure MUST push one component onto each column that `target` has and
    /// this table lacks. A move that adds no columns takes a `fill` that does nothing, written
    /// `|_, _| {}`.
    ///
    /// The last row in this table then fills `row`, in the entity list and in every column.
    ///
    /// The method returns the entity that now occupies `row`, or `None` where `row` was the last row,
    /// since every other entity then keeps its row. The moved entity now occupies row
    /// `target.len() - 1`. The caller MUST update the location that it records for both entities, as
    /// the module documentation sets out.
    ///
    /// # Panics
    ///
    /// Panics where `row` is not below the length of the entity list, before anything changes. Also
    /// panics where any column in `target` lacks a component for the new row once `fill` returns.
    ///
    /// # Aborts
    ///
    /// The program aborts where any panic occurs once the move has started and before `fill` runs,
    /// such as a panic in the `Drop` implementation of a component that `take` drops. The panic
    /// message prints
    /// first. The method moves one column at a time. Such a panic would therefore leave columns of
    /// different lengths, in this table and in `target`. Ending the program stops any later call
    /// from using either table in that state.
    ///
    /// # Examples
    ///
    /// The [module example](self#examples) moves an entity to a table that also stores `Health`,
    /// with a `fill` that pushes its new `Health`.
    pub(crate) fn move_entity(
        &mut self,
        row: usize,
        target: &mut Archetype,
        mut take: impl FnMut(&mut ComponentVec, usize),
        fill: impl FnOnce(&mut Archetype, usize),
    ) -> Option<Entity> {
        // Panics before anything changes, where `row` lies past the last entity
        assert!(
            row < self.entities.len(),
            "the row must be below the number of entities"
        );
        // Panics in a debug build, where a column and the entity list differ in length
        debug_assert!(
            self.has_one_component_per_entity(),
            "every column must have one component for each row"
        );
        // Panics in a debug build, where a target column and the target entity list differ in
        // length
        debug_assert!(
            target.has_one_component_per_entity(),
            "every target column must have one component for each row"
        );

        // Ends the program where anything below panics, since the columns in both tables would then
        // differ in length
        let guard = AbortOnUnwind;
        // Visits each column in this table
        for components in &mut self.components {
            // Finds the component type that the column stores
            let type_id = components.info().type_id();
            // Moves the entity's component where the target stores its type, and hands the column to
            // `take` otherwise
            match target.components_of_mut(type_id) {
                // Moves the component to the end of the target's column for the same type
                Some(target_components) => components.move_component(row, target_components),
                // Lets `take` remove the component, since the target has no column for its type
                None => take(components, row),
            }
        }
        // Moves the entity itself from this list to the end of the target list, filling `row` with
        // the last entity in this list
        let entity = self.entities.swap_remove(row);
        target.entities.push(entity);
        // Disarms the guard, since every component of the entity is now moved or taken
        mem::forget(guard);

        let new_row = target.entities.len() - 1;
        fill(target, new_row);

        // Panics where `fill` left a new target column without a component for the new row
        assert!(
            target.has_one_component_per_entity(),
            "the fill must push one component onto every new target column"
        );

        // Returns the entity that now occupies `row`, or `None` where `row` was the last row
        self.entities.get(row).copied()
    }

    // ========================================================================================== \\
    //                                       Helper Methods                                       \\
    // ========================================================================================== \\

    /// Returns `true` where every column has one component for each entity in the entity list, and
    /// `false` where any column is longer or shorter than the entity list.
    ///
    /// # Examples
    ///
    /// For a table with one entity and one `Health`, the method returns `true`. Where a second
    /// entity joins the entity list without a `Health`, it returns `false`.
    fn has_one_component_per_entity(&self) -> bool {
        let ent_count = self.entities.len();
        // Checks that every column has one component for each row
        self.components
            .iter()
            .all(|components| components.len() == ent_count)
    }

    /// Returns the position of the column for `type_id` in `components`, or `None` where the table
    /// has no such column.
    ///
    /// # Examples
    ///
    /// In a table for `{Position, Health}`, the method returns `Some(0)` for `Health` where the
    /// `TypeId` of `Health` sorts before the one for `Position`, and `Some(1)` otherwise. It
    /// returns `None` for `Velocity`, which the table lacks.
    fn find_index_of(&self, type_id: TypeId) -> Option<usize> {
        self.type_ids.binary_search(&type_id).ok()
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Ends the program when dropped, which happens only where a panic unwinds past it.
///
/// A method creates the guard before a change that, part of the way through, leaves the columns of
/// a table with different lengths. The method then passes the guard to [`mem::forget`] once the
/// change is complete. A panic before that point unwinds the stack, which drops the guard and
/// thereby aborts the program. Any code that would catch the panic never runs, meaning nothing can
/// use the table while its columns differ in length.
///
/// The abort follows a decision that `ComponentVec` also takes. That decision treats a panicking
/// `Drop` as a bug in the component.
///
/// # Examples
///
/// [`Archetype::remove_entity`] shows the pattern. It creates the guard before the first column
/// changes length, removes the row from each column and from the entity list, then passes the guard
/// to `mem::forget`. A panic between those two points drops the guard, and thereby aborts the
/// program.
struct AbortOnUnwind;

impl Drop for AbortOnUnwind {
    /// Aborts the program. The panic hook has already printed the panic message by this point.
    ///
    /// The docs on [`AbortOnUnwind`] describe the one path that reaches this method, a panic
    /// between creating the guard and passing it to [`mem::forget`].
    fn drop(&mut self) {
        process::abort();
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[derive(Debug, PartialEq)]
    struct Position(f32);

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    /// Adds 1 to a shared counter when dropped, which shows how many times clean-up code ran.
    ///
    /// Each `DropCounter` stores an `Arc`, a pointer that shares one counter between several
    /// owners. The counter is freed once the last owner is dropped.
    struct DropCounter {
        drops: Arc<AtomicUsize>,
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Pushes `value` onto the column that stores `T`, which leaves the entity list unchanged.
    fn push_component<T: 'static>(table: &mut Archetype, value: T) {
        let column = table
            .components_of_mut(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.push_typed(value);
    }

    /// Adds one row to a table whose only column stores `T`.
    fn push_row<T: 'static>(table: &mut Archetype, entity: Entity, value: T) {
        table.push_entity(entity, |table, _| push_component(table, value));
    }

    /// Adds one row to a table whose two columns store `A` and `B`.
    fn push_pair_row<A: 'static, B: 'static>(table: &mut Archetype, entity: Entity, a: A, b: B) {
        table.push_entity(entity, |table, _| {
            push_component(table, a);
            push_component(table, b);
        });
    }

    /// Returns the column that stores `T`, as a slice of `T`.
    ///
    /// The slice borrows `table`, meaning the compiler rejects any change to the table while the
    /// slice is in use.
    fn read_column<T: 'static>(table: &Archetype) -> &[T] {
        let column = table
            .components_of(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.as_slice::<T>()
    }

    /// Checks the starting state of a table, from no columns up to a bit at the top of the mask
    #[test]
    fn new_with_varying_infos_returns_empty_table_with_combined_mask() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let name = ComponentInfo::of::<String>(ComponentMask::single(63));

        // Lists the entries, the number of columns they should give, and the mask they should give
        let cases = [
            ("no entries", vec![], 0, ComponentMask::EMPTY),
            (
                "one entry",
                vec![position],
                1,
                ComponentMask::from_positions(&[0]),
            ),
            (
                "two entries",
                vec![position, health],
                2,
                ComponentMask::from_positions(&[0, 1]),
            ),
            (
                "highest bit",
                vec![position, name],
                2,
                ComponentMask::from_positions(&[0, 63]),
            ),
        ];

        for (label, infos, columns, mask) in cases {
            let table = Archetype::new(&infos);

            assert_eq!(table.len(), 0, "length for {label}");
            assert_eq!(table.infos().len(), columns, "columns for {label}");
            assert_eq!(table.mask(), mask, "mask for {label}");
        }
    }

    /// Checks that a shared bit panics, since queries would then treat two types as one
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn new_with_shared_bit_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));

        Archetype::new(&[position, health]);
    }

    /// Checks that the entry order makes no difference, and that each bit stays with its type
    #[test]
    fn infos_with_any_entry_order_returns_each_type_and_bit_sorted_by_type_id() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let name = ComponentInfo::of::<String>(ComponentMask::single(2));

        let mut expected = vec![
            (TypeId::of::<Position>(), ComponentMask::single(0)),
            (TypeId::of::<Health>(), ComponentMask::single(1)),
            (TypeId::of::<String>(), ComponentMask::single(2)),
        ];
        expected.sort_by_key(|(type_id, _)| *type_id);

        let cases = [
            [position, health, name],
            [name, health, position],
            [health, name, position],
        ];

        for (case, infos) in cases.iter().enumerate() {
            let table = Archetype::new(infos);

            let found: Vec<(TypeId, ComponentMask)> = table
                .infos()
                .iter()
                .map(|info| (info.type_id(), info.bit()))
                .collect();

            assert_eq!(found, expected, "infos for entry order {case}");
        }
    }

    /// Checks that the count follows the entity list, from an empty table upwards
    #[test]
    fn len_with_varying_row_counts_returns_row_count() {
        let cases = [0_u32, 1, 5];

        for count in cases {
            let mut table =
                Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
            for index in 0..count {
                push_row(&mut table, Entity::new(index, 0), Health(index));
            }

            assert_eq!(table.len(), count as usize, "length for {count} rows");
        }
    }

    /// Checks the binary search at every table size, for both the types stored and the types absent
    #[test]
    fn components_of_with_stored_and_absent_types_finds_only_stored_columns() {
        let infos = [
            ComponentInfo::of::<u8>(ComponentMask::single(0)),
            ComponentInfo::of::<u16>(ComponentMask::single(1)),
            ComponentInfo::of::<u32>(ComponentMask::single(2)),
            ComponentInfo::of::<u64>(ComponentMask::single(3)),
            ComponentInfo::of::<String>(ComponentMask::single(4)),
        ];

        for count in 0..=infos.len() {
            let (stored, absent) = infos.split_at(count);
            let table = Archetype::new(stored);

            for info in stored {
                let found = table
                    .components_of(info.type_id())
                    .map(|column| column.info().type_id());

                assert_eq!(found, Some(info.type_id()), "stored type, {count} columns");
            }
            for info in absent {
                let found = table.components_of(info.type_id());

                assert!(found.is_none(), "absent type, {count} columns");
            }
        }
    }

    /// Checks that a push through the returned column reaches that column and no other
    #[test]
    fn components_of_mut_with_stored_type_returns_column_that_keeps_pushes() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        let column = table
            .components_of_mut(position.type_id())
            .expect("the table should store `Position`");
        column.push_typed(Position(1.5));

        assert_eq!(
            read_column::<Position>(&table),
            [Position(1.5)],
            "the column that took the push"
        );
        assert!(read_column::<Health>(&table).is_empty(), "the other column");
    }

    /// Checks that a type outside the table gives no column to change
    #[test]
    fn components_of_mut_with_absent_type_returns_none() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Position>(ComponentMask::single(0))]);

        let found = table.components_of_mut(TypeId::of::<Health>());

        assert!(found.is_none());
    }

    /// Checks the query masks that match a table, along with the ones that ask for too much
    #[test]
    fn has_all_with_varying_query_masks_returns_expected_answer() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(2));
        let table = Archetype::new(&[position, health]);

        // Pairs each query mask with the answer that it should give
        let every_position: Vec<usize> = (0..ComponentMask::CAPACITY).collect();
        let cases = [
            (ComponentMask::from_positions(&[0, 2]), true),
            (ComponentMask::from_positions(&[0]), true),
            (ComponentMask::from_positions(&[2]), true),
            (ComponentMask::EMPTY, true),
            (ComponentMask::from_positions(&[1]), false),
            (ComponentMask::from_positions(&[0, 1, 2]), false),
            (ComponentMask::from_positions(&every_position), false),
        ];

        for (query_mask, expected) in cases {
            assert_eq!(
                table.has_all(query_mask),
                expected,
                "answer for query mask {query_mask:#b}"
            );
        }
    }

    /// Checks that a table with no columns matches only the query that asks for no types
    #[test]
    fn has_all_with_empty_table_matches_only_zero_mask() {
        let table = Archetype::new(&[]);

        let matches_zero = table.has_all(ComponentMask::EMPTY);
        let matches_one = table.has_all(ComponentMask::single(0));

        assert!(matches_zero, "answer for query mask 0");
        assert!(!matches_one, "answer for query mask 1");
    }

    /// Checks that entities keep the order of their pushes, including the largest index and version
    #[test]
    fn push_entity_with_varying_entities_keeps_row_order() {
        let entities = [
            Entity::new(0, 0),
            Entity::new(1, 0),
            Entity::new(1, 1),
            Entity::new(u32::MAX, u32::MAX),
        ];
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);

        for (row, entity) in entities.into_iter().enumerate() {
            push_row(&mut table, entity, Health(row as u32));
        }

        assert_eq!(table.entities, entities);
    }

    /// Checks that the entity and its component share row 0, where the entity goes in before its
    /// component
    #[test]
    fn push_entity_with_component_pushed_afterwards_shares_row_with_component() {
        let entity = Entity::new(0, 0);
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);

        table.push_entity(entity, |table, _| push_component(table, Health(100)));

        assert_eq!(table.entities, [entity], "entity list");
        assert_eq!(read_column::<Health>(&table), [Health(100)], "column");
    }

    /// Checks that the last row fills the empty row, in the entity list and in the column
    #[test]
    fn move_entity_with_middle_row_fills_gap_with_last_row() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        let entities = [
            Entity::new(0, 0),
            Entity::new(1, 0),
            Entity::new(2, 0),
            Entity::new(3, 0),
        ];
        let [first, second, third, fourth] = entities;
        for (entity, x) in entities.into_iter().zip([1.0, 2.0, 3.0, 4.0]) {
            push_row(&mut source, entity, Position(x));
        }

        let filled = source.move_entity(1, &mut target, ComponentVec::swap_remove, |_, _| {});

        assert_eq!(filled, Some(fourth), "entity that fills the row");
        assert_eq!(source.entities, [first, fourth, third], "source entities");
        assert_eq!(
            read_column::<Position>(&source),
            [Position(1.0), Position(4.0), Position(3.0)],
            "source column"
        );
        assert_eq!(target.entities, [second], "target entities");
        assert_eq!(
            read_column::<Position>(&target),
            [Position(2.0)],
            "target column"
        );
    }

    /// Checks that moving the last row does not change any other row, and therefore returns `None`
    #[test]
    fn move_entity_with_last_row_returns_none() {
        let cases = [1_u32, 2, 5];

        for count in cases {
            let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
            let mut source = Archetype::new(&[position]);
            let mut target = Archetype::new(&[position]);
            for index in 0..count {
                push_row(&mut source, Entity::new(index, 0), Position(index as f32));
            }
            let last = count - 1;

            let filled = source.move_entity(
                last as usize,
                &mut target,
                ComponentVec::swap_remove,
                |_, _| {},
            );

            let remaining = last as usize;
            assert_eq!(filled, None, "entity that fills the row, {count} rows");
            assert_eq!(source.len(), remaining, "source length, {count} rows");
            assert_eq!(
                read_column::<Position>(&source).len(),
                remaining,
                "source column length, {count} rows"
            );
            assert_eq!(
                target.entities,
                [Entity::new(last, 0)],
                "target entities, {count} rows"
            );
        }
    }

    /// Checks that a moved entity joins the end of the target, after the rows already there
    #[test]
    fn move_entity_with_occupied_target_adds_row_at_end() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        let moved = Entity::new(0, 0);
        let resident = Entity::new(1, 0);
        push_row(&mut source, moved, Position(1.0));
        push_row(&mut target, resident, Position(2.0));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});

        assert_eq!(target.entities, [resident, moved], "target entities");
        assert_eq!(
            read_column::<Position>(&target),
            [Position(2.0), Position(1.0)],
            "target column"
        );
    }

    /// Checks that a move into a full target grows its column, and keeps its rows in order
    #[test]
    fn move_entity_with_full_target_keeps_rows_in_order() {
        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        // Fills the target column to its first capacity of 4 components
        for index in 0..4_u32 {
            push_row(&mut target, Entity::new(index, 0), Health(index));
        }
        let moved = Entity::new(4, 0);
        push_row(&mut source, moved, Health(4));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});

        let moved_row = target.len() - 1;
        assert_eq!(target.entities[moved_row], moved, "last target entity");
        assert_eq!(
            read_column::<Health>(&target),
            [Health(0), Health(1), Health(2), Health(3), Health(4)],
            "target column"
        );
    }

    /// Checks that repeated moves report each filling entity, and empty the source
    #[test]
    fn move_entity_with_repeated_moves_empties_source() {
        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);
        push_row(&mut source, first, Health(1));
        push_row(&mut source, second, Health(2));
        push_row(&mut source, third, Health(3));

        let filled = [
            source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {}),
            source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {}),
            source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {}),
        ];

        assert_eq!(
            filled,
            [Some(third), Some(second), None],
            "filling entities"
        );
        assert_eq!(source.len(), 0, "source length");
        assert_eq!(target.entities, [first, third, second], "target entities");
        assert_eq!(
            read_column::<Health>(&target),
            [Health(1), Health(3), Health(2)],
            "target column"
        );
    }

    /// Checks that a whole row moves together, as the entity gains a component in a larger table
    #[test]
    fn move_entity_with_several_columns_keeps_rows_together() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let name = ComponentInfo::of::<String>(ComponentMask::single(1));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(2));

        let mut source = Archetype::new(&[position, name]);
        let mut target = Archetype::new(&[name, health, position]);

        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);

        for (entity, x, label) in [(first, 1.0, "Ada"), (second, 2.0, "Bo"), (third, 3.0, "Cy")] {
            push_pair_row(&mut source, entity, Position(x), String::from(label));
        }

        let filled = source.move_entity(0, &mut target, ComponentVec::swap_remove, |table, _| {
            push_component(table, Health(100))
        });

        assert_eq!(filled, Some(third), "entity that fills the row");
        assert_eq!(source.entities, [third, second], "source entities");
        assert_eq!(
            read_column::<Position>(&source),
            [Position(3.0), Position(2.0)],
            "source positions"
        );
        assert_eq!(read_column::<String>(&source), ["Cy", "Bo"], "source names");
        assert_eq!(target.entities, [first], "target entities");
        assert_eq!(
            read_column::<Position>(&target),
            [Position(1.0)],
            "target positions"
        );
        assert_eq!(read_column::<String>(&target), ["Ada"], "target names");
        assert_eq!(
            read_column::<Health>(&target),
            [Health(100)],
            "target healths"
        );
    }

    /// Checks that a table with no columns still moves its entities
    #[test]
    fn move_entity_with_no_columns_moves_only_entity() {
        let mut source = Archetype::new(&[]);
        let mut target = Archetype::new(&[]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);

        source.push_entity(first, |_, _| {});
        source.push_entity(second, |_, _| {});

        let filled = source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});

        assert_eq!(filled, Some(second), "entity that fills the row");
        assert_eq!(source.entities, [second], "source entities");
        assert_eq!(target.entities, [first], "target entities");
    }

    /// Checks that a column of a zero-sized type moves by count alone
    #[test]
    fn move_entity_with_zero_size_type_moves_count() {
        struct Tag;

        let tag = ComponentInfo::of::<Tag>(ComponentMask::single(0));
        let mut source = Archetype::new(&[tag]);
        let mut target = Archetype::new(&[tag]);
        push_row(&mut source, Entity::new(0, 0), Tag);
        push_row(&mut source, Entity::new(1, 0), Tag);

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});

        assert_eq!(read_column::<Tag>(&source).len(), 1, "source column length");
        assert_eq!(read_column::<Tag>(&target).len(), 1, "target column length");
    }

    /// Checks that a move passes ownership on, meaning two tables never clean up the same component
    #[test]
    fn move_entity_with_drop_counters_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let counter = ComponentInfo::of::<DropCounter>(ComponentMask::single(0));
        let mut source = Archetype::new(&[counter]);
        let mut target = Archetype::new(&[counter]);
        for index in 0..3 {
            let value = DropCounter {
                drops: Arc::clone(&drops),
            };
            push_row(&mut source, Entity::new(index, 0), value);
        }

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
        let drops_after_move = drops.load(Ordering::Relaxed);

        drop(source);
        drop(target);
        let drops_after_tables = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_move, 0, "drops caused by the move");
        assert_eq!(drops_after_tables, 3, "drops after both tables are gone");
    }

    /// Checks that every build catches a row past the end of the columns
    #[test]
    #[should_panic]
    fn move_entity_with_out_of_range_row_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        source.move_entity(1, &mut target, ComponentVec::swap_remove, |_, _| {});
    }

    /// Checks that a debug build catches an entity list shorter than its columns, before anything
    /// changes
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn move_entity_with_short_entity_list_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_component(&mut source, Position(1.0));
        push_component(&mut source, Position(2.0));
        // Adds one entity for the two components, which `push_entity` would refuse
        source.entities.push(Entity::new(0, 0));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
    }

    /// Checks that every build catches a `fill` that leaves a column without a component for the
    /// new row
    #[test]
    #[should_panic]
    fn push_entity_with_fill_missing_column_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        table.push_entity(Entity::new(0, 0), |table, _| {
            push_component(table, Position(1.0));
        });
    }

    /// Checks that every build catches a `fill` that leaves a new target column without a
    /// component for the moved entity
    #[test]
    #[should_panic]
    fn move_entity_with_fill_missing_new_column_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position, health]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
    }

    /// Checks that dropping a table runs the clean-up code once for each component it stores
    #[test]
    fn drop_with_varying_row_counts_drops_each_component_once() {
        let cases = [0_u32, 1, 5];

        for count in cases {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut table =
                Archetype::new(&[ComponentInfo::of::<DropCounter>(ComponentMask::single(0))]);
            for index in 0..count {
                let value = DropCounter {
                    drops: Arc::clone(&drops),
                };
                push_row(&mut table, Entity::new(index, 0), value);
            }

            drop(table);

            assert_eq!(
                drops.load(Ordering::Relaxed),
                count as usize,
                "drops for {count} rows"
            );
        }
    }

    /// Checks that threads can read one table together, and that a table can move to a thread
    #[test]
    fn components_of_with_several_threads_returns_same_components() {
        let mut table = Archetype::new(&[ComponentInfo::of::<u32>(ComponentMask::single(0))]);
        for value in [1_u32, 2, 3, 4] {
            push_row(&mut table, Entity::new(value, 0), value);
        }

        // Adds up the components, borrowing the table rather than taking ownership of it
        let read_total = || read_column::<u32>(&table).iter().sum::<u32>();

        let shared_totals = thread::scope(|scope| {
            let handles = [scope.spawn(read_total), scope.spawn(read_total)];
            handles.map(|handle| handle.join().unwrap())
        });

        let moved_total = thread::spawn(move || read_column::<u32>(&table).iter().sum::<u32>())
            .join()
            .unwrap();

        assert_eq!(shared_totals, [10, 10], "totals from the shared table");
        assert_eq!(moved_total, 10, "total from the moved table");
    }

    /// Checks that the entity list comes back in row order, from an empty table upwards
    #[test]
    fn entities_with_varying_row_counts_returns_entities_in_row_order() {
        let cases = [0_u32, 1, 3];

        for count in cases {
            let mut table =
                Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
            let expected: Vec<Entity> = (0..count).map(|index| Entity::new(index, 1)).collect();
            for (index, entity) in (0..count).zip(expected.iter().copied()) {
                push_row(&mut table, entity, Health(index));
            }

            let found = table.entities();

            assert_eq!(found, expected.as_slice(), "entities for {count} rows");
        }
    }

    /// Checks that reserving room leaves every row in place, in the entity list and in each column
    #[test]
    fn reserve_with_varying_additional_rows_keeps_existing_rows() {
        let cases = [0_usize, 1, 100];

        for additional in cases {
            let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
            let name = ComponentInfo::of::<String>(ComponentMask::single(1));
            let mut table = Archetype::new(&[position, name]);
            let first = Entity::new(0, 0);
            let second = Entity::new(1, 0);
            push_pair_row(&mut table, first, Position(1.0), String::from("Ada"));
            push_pair_row(&mut table, second, Position(2.0), String::from("Bo"));

            table.reserve(additional);

            assert_eq!(
                table.entities(),
                [first, second],
                "entities after reserving {additional}"
            );
            assert_eq!(
                read_column::<Position>(&table),
                [Position(1.0), Position(2.0)],
                "positions after reserving {additional}"
            );
            assert_eq!(
                read_column::<String>(&table),
                ["Ada", "Bo"],
                "names after reserving {additional}"
            );
        }
    }

    /// Checks that the reserved rows fit in the blocks that the table already has, meaning no
    /// column and not the entity list moves to a larger block
    #[test]
    fn reserve_with_varying_additional_rows_fits_rows_without_moving_blocks() {
        let cases = [1_u32, 4, 9];

        for additional in cases {
            let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
            let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
            let mut table = Archetype::new(&[position, health]);
            push_pair_row(&mut table, Entity::new(0, 0), Position(0.0), Health(0));

            table.reserve(additional as usize);
            let reserved_positions = read_column::<Position>(&table).as_ptr();
            let reserved_healths = read_column::<Health>(&table).as_ptr();
            let reserved_entities = table.entities().as_ptr();
            for index in 1..=additional {
                let row = Entity::new(index, 0);
                push_pair_row(&mut table, row, Position(index as f32), Health(index));
            }
            let final_positions = read_column::<Position>(&table).as_ptr();
            let final_healths = read_column::<Health>(&table).as_ptr();
            let final_entities = table.entities().as_ptr();

            let rows = additional as usize + 1;
            assert_eq!(table.len(), rows, "length after {additional} pushes");
            assert_eq!(
                final_positions, reserved_positions,
                "position block after {additional} pushes"
            );
            assert_eq!(
                final_healths, reserved_healths,
                "health block after {additional} pushes"
            );
            assert_eq!(
                final_entities, reserved_entities,
                "entity block after {additional} pushes"
            );
        }
    }

    /// Checks that a request for more rows than a `usize` can count panics
    #[test]
    #[should_panic]
    fn reserve_with_overflowing_additional_rows_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
        push_row(&mut table, Entity::new(0, 0), Health(1));

        table.reserve(usize::MAX);
    }

    /// Checks that the borrowed columns come back in the order of the request, whatever the order
    /// of the columns inside the table
    #[test]
    fn get_disjoint_components_mut_with_stored_types_returns_columns_in_requested_order() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let name = ComponentInfo::of::<String>(ComponentMask::single(2));

        // Lists the two types to request, in the order that the columns should come back
        let cases = [
            [position.type_id(), health.type_id()],
            [health.type_id(), position.type_id()],
            [name.type_id(), position.type_id()],
        ];

        for (case, type_ids) in cases.into_iter().enumerate() {
            let mut table = Archetype::new(&[position, health, name]);

            let found = table
                .get_disjoint_components_mut(type_ids)
                .map(|columns| columns.map(|column| column.info().type_id()));

            assert_eq!(found, Some(type_ids), "columns for request {case}");
        }
    }

    /// Checks that each borrowed column takes its own push, which the table then keeps
    #[test]
    fn get_disjoint_components_mut_with_stored_types_returns_columns_that_keep_pushes() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        let [healths, positions] = table
            .get_disjoint_components_mut([health.type_id(), position.type_id()])
            .expect("the table should store `Health` and `Position`");
        healths.push_typed(Health(100));
        positions.push_typed(Position(1.5));

        assert_eq!(read_column::<Health>(&table), [Health(100)], "healths");
        assert_eq!(
            read_column::<Position>(&table),
            [Position(1.5)],
            "positions"
        );
    }

    /// Checks that a request for a type outside the table gives no columns, wherever that type
    /// stands in the request
    #[test]
    fn get_disjoint_components_mut_with_absent_type_returns_none() {
        let position = TypeId::of::<Position>();
        let health = TypeId::of::<Health>();
        let name = TypeId::of::<String>();

        // Lists requests that each contain at least one type outside the table
        let cases = [[name, position], [health, name], [name, name]];

        for (case, type_ids) in cases.into_iter().enumerate() {
            let mut table = Archetype::new(&[
                ComponentInfo::of::<Position>(ComponentMask::single(0)),
                ComponentInfo::of::<Health>(ComponentMask::single(1)),
            ]);

            let found = table.get_disjoint_components_mut(type_ids);

            assert!(found.is_none(), "answer for request {case}");
        }
    }

    /// Checks that a request for one column twice gives no columns, since the caller could then
    /// change that column through two references at once
    #[test]
    fn get_disjoint_components_mut_with_repeated_type_returns_none() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        let found = table.get_disjoint_components_mut([
            position.type_id(),
            health.type_id(),
            position.type_id(),
        ]);

        assert!(found.is_none());
    }

    /// Checks the smallest requests, which ask for no columns or for a single column
    #[test]
    fn get_disjoint_components_mut_with_zero_or_one_type_returns_requested_columns() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut table = Archetype::new(&[position]);
        let mut empty_table = Archetype::new(&[]);

        let no_types = table
            .get_disjoint_components_mut([])
            .map(|columns| columns.len());
        let no_types_empty_table = empty_table
            .get_disjoint_components_mut([])
            .map(|columns| columns.len());
        let one_type = table
            .get_disjoint_components_mut([position.type_id()])
            .map(|[column]| column.info().type_id());

        assert_eq!(no_types, Some(0), "no types from a table with one column");
        assert_eq!(
            no_types_empty_table,
            Some(0),
            "no types from a table with no columns"
        );
        assert_eq!(one_type, Some(position.type_id()), "one type");
    }

    /// Checks that the last row fills the removed row, for the first, middle and last rows, and
    /// for the only row
    #[test]
    fn remove_entity_with_varying_rows_fills_gap_with_last_row() {
        // Lists the number of rows, the row to remove, the index of the entity that should fill
        // it, and the indices of the entities that should remain, in row order
        let cases: [(u32, usize, Option<u32>, &[u32]); 4] = [
            (3, 0, Some(2), &[2, 1]),
            (3, 1, Some(2), &[0, 2]),
            (3, 2, None, &[0, 1]),
            (1, 0, None, &[]),
        ];

        for (count, row, filled, remaining) in cases {
            let mut table =
                Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
            for index in 0..count {
                push_row(&mut table, Entity::new(index, 0), Health(index));
            }
            let expected_filled = filled.map(|index| Entity::new(index, 0));
            let expected_entities: Vec<Entity> = remaining
                .iter()
                .map(|&index| Entity::new(index, 0))
                .collect();
            let expected_healths: Vec<Health> =
                remaining.iter().map(|&index| Health(index)).collect();

            let found = table.remove_entity(row);

            assert_eq!(
                found, expected_filled,
                "entity that fills the row, row {row} of {count}"
            );
            assert_eq!(
                table.entities(),
                expected_entities.as_slice(),
                "entities, row {row} of {count}"
            );
            assert_eq!(
                read_column::<Health>(&table),
                expected_healths.as_slice(),
                "column, row {row} of {count}"
            );
        }
    }

    /// Checks that a whole row leaves together, and that the last row fills it in every column
    #[test]
    fn remove_entity_with_several_columns_keeps_rows_together() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let name = ComponentInfo::of::<String>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, name]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);
        push_pair_row(&mut table, first, Position(1.0), String::from("Ada"));
        push_pair_row(&mut table, second, Position(2.0), String::from("Bo"));
        push_pair_row(&mut table, third, Position(3.0), String::from("Cy"));

        let filled = table.remove_entity(0);

        assert_eq!(filled, Some(third), "entity that fills the row");
        assert_eq!(table.entities(), [third, second], "entities");
        assert_eq!(
            read_column::<Position>(&table),
            [Position(3.0), Position(2.0)],
            "positions"
        );
        assert_eq!(read_column::<String>(&table), ["Cy", "Bo"], "names");
    }

    /// Checks that a table with no columns still removes its entities
    #[test]
    fn remove_entity_with_no_columns_removes_only_entity() {
        let mut table = Archetype::new(&[]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        table.push_entity(first, |_, _| {});
        table.push_entity(second, |_, _| {});

        let filled = table.remove_entity(0);

        assert_eq!(filled, Some(second), "entity that fills the row");
        assert_eq!(table.entities(), [second], "entities");
    }

    /// Checks that a removal drops the removed component alone, and that the table later drops the
    /// rest once each
    #[test]
    fn remove_entity_with_drop_counters_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut table =
            Archetype::new(&[ComponentInfo::of::<DropCounter>(ComponentMask::single(0))]);
        for index in 0..3 {
            let value = DropCounter {
                drops: Arc::clone(&drops),
            };
            push_row(&mut table, Entity::new(index, 0), value);
        }

        table.remove_entity(0);
        let drops_after_removal = drops.load(Ordering::Relaxed);

        drop(table);
        let drops_after_table = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_removal, 1, "drops caused by the removal");
        assert_eq!(drops_after_table, 3, "drops after the table is gone");
    }

    /// Checks that a column of a zero-sized type with clean-up code drops the removed component
    #[test]
    fn remove_entity_with_zero_size_drop_type_drops_removed_component() {
        // Counts the drops outside the type, since a type of size zero has no room for a counter
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Marker;
        impl Drop for Marker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut table = Archetype::new(&[ComponentInfo::of::<Marker>(ComponentMask::single(0))]);
        push_row(&mut table, Entity::new(0, 0), Marker);
        push_row(&mut table, Entity::new(1, 0), Marker);

        table.remove_entity(1);
        let column_length = read_column::<Marker>(&table).len();
        let drops_after_removal = DROPS.load(Ordering::Relaxed);

        drop(table);
        let drops_after_table = DROPS.load(Ordering::Relaxed);

        assert_eq!(column_length, 1, "column length");
        assert_eq!(drops_after_removal, 1, "drops caused by the removal");
        assert_eq!(drops_after_table, 2, "drops after the table is gone");
    }

    /// Checks that every build catches a row past the last entity
    #[test]
    #[should_panic]
    fn remove_entity_with_out_of_range_row_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
        push_row(&mut table, Entity::new(0, 0), Health(1));

        table.remove_entity(1);
    }

    /// Checks that every build catches a removal from a table with no rows
    #[test]
    #[should_panic]
    fn remove_entity_with_empty_table_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);

        table.remove_entity(0);
    }

    /// Checks that a debug build catches a column shorter than the entity list, before anything
    /// changes
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn remove_entity_with_short_column_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
        push_row(&mut table, Entity::new(0, 0), Health(1));
        // Adds an entity without its component, which `push_entity` would refuse
        table.entities.push(Entity::new(1, 0));

        table.remove_entity(0);
    }

    /// Checks that a move into a table without a column for one component drops that component,
    /// and moves the rest of the row
    #[test]
    fn move_entity_with_target_lacking_column_drops_that_component() {
        let drops = Arc::new(AtomicUsize::new(0));
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let counter = ComponentInfo::of::<DropCounter>(ComponentMask::single(1));
        let mut source = Archetype::new(&[position, counter]);
        let mut target = Archetype::new(&[position]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);
        for (entity, x) in [(first, 1.0), (second, 2.0), (third, 3.0)] {
            let value = DropCounter {
                drops: Arc::clone(&drops),
            };
            push_pair_row(&mut source, entity, Position(x), value);
        }

        let filled = source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
        let drops_after_move = drops.load(Ordering::Relaxed);
        let counters_left = source
            .components_of(counter.type_id())
            .expect("the source should store `DropCounter`")
            .len();

        drop(source);
        let drops_after_source = drops.load(Ordering::Relaxed);

        assert_eq!(filled, Some(third), "entity that fills the row");
        assert_eq!(drops_after_move, 1, "drops caused by the move");
        assert_eq!(counters_left, 2, "counters left in the source");
        assert_eq!(drops_after_source, 3, "drops after the source is gone");
        assert_eq!(target.entities(), [first], "target entities");
        assert_eq!(
            read_column::<Position>(&target),
            [Position(1.0)],
            "target positions"
        );
    }

    /// Checks that a move into a table with no columns drops every component of the entity
    #[test]
    fn move_entity_with_columnless_target_drops_every_component() {
        let drops = Arc::new(AtomicUsize::new(0));
        let counter = ComponentInfo::of::<DropCounter>(ComponentMask::single(0));
        let mut source = Archetype::new(&[counter]);
        let mut target = Archetype::new(&[]);
        let moved = Entity::new(0, 0);
        let value = DropCounter {
            drops: Arc::clone(&drops),
        };
        push_row(&mut source, moved, value);

        let filled = source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
        let drops_after_move = drops.load(Ordering::Relaxed);

        assert_eq!(filled, None, "entity that fills the row");
        assert_eq!(drops_after_move, 1, "drops caused by the move");
        assert_eq!(source.len(), 0, "source length");
        assert_eq!(
            read_column::<DropCounter>(&source).len(),
            0,
            "source column length"
        );
        assert_eq!(target.entities(), [moved], "target entities");
    }

    /// Checks that a debug build catches a source column shorter than the entity list, before
    /// anything changes
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn move_entity_with_short_source_column_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        // Adds an entity without its component, which `push_entity` would refuse
        source.entities.push(Entity::new(1, 0));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
    }

    /// Checks that a debug build catches a target column shorter than the target entity list,
    /// before anything changes
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn move_entity_with_short_target_column_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        // Adds an entity to the target without its component, which `push_entity` would refuse
        target.entities.push(Entity::new(1, 0));

        source.move_entity(0, &mut target, ComponentVec::swap_remove, |_, _| {});
    }
}
