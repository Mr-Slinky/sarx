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
//! The caller adds a row in two steps. It pushes one component onto every column, using
//! [`Archetype::mut_components_of`]. It then pushes the entity with [`Archetype::push_entity`].
//! The two steps can run in either order. Once both are finished, every column again has one
//! component for each entity in the list.
//!
//! # Masks
//!
//! A table has a mask, which is a [`ComponentMask`] with one set bit for each component type that it
//! stores.
//! Each bit comes from the [`ComponentInfo`] for that type, which describes the type to the table.
//!
//! A caller that wants a set of component types builds a mask of those types. It then passes that
//! mask to [`Archetype::has_all`]. The method answers whether the table stores every one of those
//! types, from the two masks alone.
//!
//! # Moves between tables
//!
//! An entity moves to another table when it gains or loses a component. The caller handles both
//! cases with [`Archetype::move_entity_dropping`]. Where the target lacks a column for one of the
//! entity's components, the method drops that component. The last row in the old table then fills
//! the empty row, which keeps each column in one unbroken run.
//!
//! The caller records a location for each entity, which is its table and its row. A move changes
//! the location of the moved entity, and of the entity that fills the empty row, where there is
//! one. The caller MUST update both. A caller that keeps a stale row can later pass an index past
//! the end of a column to `move_entity_dropping`, which then panics before anything changes.
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
//! // Adds two rows to the source table, pushing each component before its entity
//! let first = Entity::new(0, 0);
//! let second = Entity::new(1, 0);
//! for (entity, x) in [(first, 1.0), (second, 2.0)] {
//!     let positions = source
//!         .mut_components_of(position.type_id())
//!         .expect("the table should store `Position`"); // &mut ComponentVec
//!     positions.push_typed(Position(x));
//!
//!     // SAFETY: `push_entity` requires one new component in every column. The line above pushes
//!     // one onto `Position`, which is the only column
//!     unsafe { source.push_entity(entity) };
//! }
//!
//! // Moves row 0 to the target table, and fills row 0 with the last row
//! // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
//! // target. Row 0 is below the length of 2, and the two statements below push onto `Health`
//! let filled = unsafe { source.move_entity_dropping(0, &mut target) };
//!
//! // Gives the moved entity its new component
//! let healths = target.mut_components_of(health.type_id()).expect("the table should store `Health`");
//! healths.push_typed(Health(100));
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
/// [`mut_components_of`](Self::mut_components_of). The module documentation sets out how a caller
/// adds a row and moves one to another table.
pub(crate) struct Archetype {
    /// Stores one column for each component type, sorted by `TypeId`.
    components: Vec<ComponentVec>,
    /// Stores the entity that owns each row, in row order.
    entities: Vec<Entity>,
    /// Stores one set bit for each component type in `components`.
    mask: ComponentMask,
}

impl Archetype {
    /// Returns an empty table with one column for each entry in `infos`. The caller can list the
    /// entries in any order.
    ///
    /// The mask combines the bit from each entry. An empty `infos` gives a table with no columns and
    /// the mask [`ComponentMask::EMPTY`].
    ///
    /// The caller MUST pass each component type once, with a bit that follows the rules in
    /// [`ComponentInfo::of`]. A type passed twice with two different bits gets two columns. A search
    /// for that type returns one column every time, and which one is unspecified. The other column
    /// is then never pushed to, and the table breaks the guarantee that
    /// [`push_entity`](Self::push_entity) requires.
    ///
    /// # Panics
    ///
    /// Panics where two entries in `infos` share a bit.
    pub(crate) fn new(infos: &[ComponentInfo]) -> Self {
        let mut components: Vec<ComponentVec> =
            infos.iter().map(|info| ComponentVec::new(*info)).collect();
        // Sorts the columns by `TypeId`, which the binary search in `find_index_of` requires
        components.sort_by_key(|components| components.info().type_id());
        let mut mask = ComponentMask::EMPTY;
        for info in infos {
            // Stops two types from sharing a bit, which would make queries treat them as one type
            assert!(
                !mask.intersects(info.bit()),
                "two component types in one table share a mask bit"
            );
            mask |= info.bit();
        }

        Self {
            components,
            entities: Vec::new(),
            mask,
        }
    }

    /// Returns the number of entities in the table.
    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns the [`ComponentInfo`] for each component type in the table, sorted by [`TypeId`].
    ///
    /// Each call builds a new `Vec`, which the caller then owns.
    pub(crate) fn infos(&self) -> Vec<ComponentInfo> {
        self.components.iter().map(ComponentVec::info).collect()
    }

    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Returns the column for `type_id`, or `None` where the table has no such column.
    ///
    /// The returned reference borrows the table, meaning the compiler rejects any change to the
    /// table while the reference is in use.
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
    /// section in the module documentation sets out. A replacement column MUST also store the same component type, since
    /// the search depends on the sorted order.
    pub(crate) fn mut_components_of(&mut self, type_id: TypeId) -> Option<&mut ComponentVec> {
        let index = self.find_index_of(type_id)?;
        Some(&mut self.components[index])
    }

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

    /// Returns the position of the column for `type_id` in `components`, or `None` where the table
    /// has no such column.
    fn find_index_of(&self, type_id: TypeId) -> Option<usize> {
        self.components
            .binary_search_by_key(&type_id, |components| components.info().type_id())
            .ok()
    }

    /// Returns the mask of the table, with one set bit for each component type that it stores.
    pub(crate) fn mask(&self) -> ComponentMask {
        self.mask
    }

    /// Returns `true` where the table stores every component type in `query_mask`, and `false` where
    /// it lacks at least one.
    ///
    /// A `query_mask` of [`ComponentMask::EMPTY`] does not ask for any component types, and the
    /// method then returns `true` for every table.
    pub(crate) fn has_all(&self, query_mask: ComponentMask) -> bool {
        self.mask.contains_all(query_mask)
    }

    /// Adds `entity` to the end of the entity list, which gives it the row `len() - 1`, measured
    /// after the call.
    ///
    /// # Panics
    ///
    /// Panics where the list is full and a larger block would exceed `isize::MAX` bytes. The program
    /// aborts where the allocator cannot supply the larger block.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee that every column has one new component for this row. The caller
    /// pushes those components through [`mut_components_of`](Self::mut_components_of), either before
    /// this call or straight after it.
    ///
    /// A missing component leaves that column one row short. Later entities then pair with the wrong
    /// components in that column. A later call to [`move_entity_dropping`](Self::move_entity_dropping)
    /// can also pass an index past the end of the column, which then ends the program, as the Aborts
    /// section of that method sets out.
    pub(crate) unsafe fn push_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Removes the entity at `row` from the table, and drops its components.
    ///
    /// The last row in this table then fills `row`, in the entity list and in every column.
    ///
    /// The method returns the entity that now occupies `row`, or `None` where `row` was the last
    /// row, since no other entity changes row. The caller MUST update the location that it records
    /// for the returned entity, as the module documentation sets out.
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

        // Ends the program where anything below panics, since the columns would then differ in length
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

    /// Moves the entity at `row` to a new row at the end of `target`, **dropping each component that
    /// `target` has no column for**.
    ///
    /// The method serves both moves between tables. Where the entity gains a component, `target`
    /// has every column in this table plus one more, and the caller fills that column afterwards.
    /// Where the entity loses a component, `target` lacks that one column, and the method drops the
    /// component.
    ///
    /// The last row in this table then fills `row`, in the entity list and in every column.
    ///
    /// The method returns the entity that now occupies `row`, or `None` where `row` was the last row,
    /// since no other entity changes row. The moved entity now occupies row `target.len() - 1`. The
    /// caller MUST update the location that it records for both entities, as the module documentation
    /// sets out.
    ///
    /// # Panics
    ///
    /// Panics where `row` is not below the length of the entity list, before anything changes.
    ///
    /// # Aborts
    ///
    /// The program aborts where any panic occurs once the move has started, such as a panic in the
    /// `Drop` implementation of a dropped component. The panic message prints first. The method
    /// moves one column at a time. Such a panic would therefore leave columns of different lengths,
    /// in this table and in `target`. Ending the program stops any later call from using either
    /// table in that state.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee two things:
    ///
    /// - `row` is below the length of every column. A column that is too short panics, and the
    ///   program then aborts, as the Aborts section sets out.
    /// - The caller pushes one component onto each column that `target` has and this table lacks,
    ///   before anything else uses `target`. A missing component leaves that column one row short,
    ///   with the effects that [`push_entity`](Self::push_entity) sets out.
    pub(crate) unsafe fn move_entity_dropping(
        &mut self,
        row: usize,
        target: &mut Archetype,
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
        // Panics in a debug build, where a target column and the target entity list differ in length
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
            // Moves the entity's component where the target stores its type, and drops it otherwise
            match target.mut_components_of(type_id) {
                // Moves the component to the end of the target's column for the same type
                // SAFETY: `move_component` requires an index below the length and a target of the same
                // type. The caller MUST guarantee that `row` is below the length of every column, and
                // `mut_components_of` returns the target column with this `TypeId`
                Some(target_components) => unsafe {
                    components.move_component(row, target_components);
                },
                // Drops the component, since the target has no column for its type
                None => components.swap_remove(row),
            }
        }
        let entity = self.entities.swap_remove(row);
        target.entities.push(entity);
        // Disarms the guard, since every component of the entity is now moved or dropped
        mem::forget(guard);

        // Returns the entity that now occupies `row`, or `None` where `row` was the last row
        self.entities.get(row).copied()
    }

    /// Returns `true` where every column has one component for each entity in the entity list, and
    /// `false` where any column is longer or shorter than the entity list.
    fn has_one_component_per_entity(&self) -> bool {
        let ent_count = self.entities.len();
        // Checks that every column has one component for each row
        self.components
            .iter()
            .all(|components| components.len() == ent_count)
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
/// The abort rests on the decision that `ComponentVec` also takes, which treats a panicking `Drop`
/// as a bug in the component.
struct AbortOnUnwind;

impl Drop for AbortOnUnwind {
    /// Aborts the program. The panic hook has already printed the panic message by this point.
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
            .mut_components_of(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.push_typed(value);
    }

    /// Adds one row to a table whose only column stores `T`.
    fn push_row<T: 'static>(table: &mut Archetype, entity: Entity, value: T) {
        // Stops a table with a second column from getting a row that the second column lacks
        assert_eq!(table.infos().len(), 1, "the table must have one column");
        push_component(table, value);

        // SAFETY: `push_entity` requires one new component in every column. The assert above leaves
        // one column, and the line above pushes onto it
        unsafe { table.push_entity(entity) };
    }

    /// Returns the column that stores `T`, as a slice of `T`.
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
    fn mut_components_of_with_stored_type_returns_column_that_keeps_pushes() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        let column = table
            .mut_components_of(position.type_id())
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
    fn mut_components_of_with_absent_type_returns_none() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Position>(ComponentMask::single(0))]);

        let found = table.mut_components_of(TypeId::of::<Health>());

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

        // SAFETY: `push_entity` requires one new component in every column. The line below pushes
        // one onto `Health`, which is the only column
        unsafe { table.push_entity(entity) };
        push_component(&mut table, Health(100));

        assert_eq!(table.entities, [entity], "entity list");
        assert_eq!(read_column::<Health>(&table), [Health(100)], "column");
    }

    /// Checks that the last row fills the empty row, in the entity list and in the column
    #[test]
    fn move_entity_dropping_with_middle_row_fills_gap_with_last_row() {
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

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 1 is below the length of 4, and the target has no new columns
        let filled = unsafe { source.move_entity_dropping(1, &mut target) };

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
    fn move_entity_dropping_with_last_row_returns_none() {
        let cases = [1_u32, 2, 5];

        for count in cases {
            let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
            let mut source = Archetype::new(&[position]);
            let mut target = Archetype::new(&[position]);
            for index in 0..count {
                push_row(&mut source, Entity::new(index, 0), Position(index as f32));
            }
            let last = count - 1;

            // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
            // target. Each case moves its last row, and the target has no new columns
            let filled = unsafe { source.move_entity_dropping(last as usize, &mut target) };

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
    fn move_entity_dropping_with_occupied_target_adds_row_at_end() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        let moved = Entity::new(0, 0);
        let resident = Entity::new(1, 0);
        push_row(&mut source, moved, Position(1.0));
        push_row(&mut target, resident, Position(2.0));

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 1, and the target has no new columns
        unsafe { source.move_entity_dropping(0, &mut target) };

        assert_eq!(target.entities, [resident, moved], "target entities");
        assert_eq!(
            read_column::<Position>(&target),
            [Position(2.0), Position(1.0)],
            "target column"
        );
    }

    /// Checks that a move into a full target grows its column, and keeps its rows in order
    #[test]
    fn move_entity_dropping_with_full_target_keeps_rows_in_order() {
        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        // Fills the target column to its first capacity of 4 components
        for index in 0..4_u32 {
            push_row(&mut target, Entity::new(index, 0), Health(index));
        }
        let moved = Entity::new(4, 0);
        push_row(&mut source, moved, Health(4));

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 1, and the target has no new columns
        unsafe { source.move_entity_dropping(0, &mut target) };

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
    fn move_entity_dropping_with_repeated_moves_empties_source() {
        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);
        push_row(&mut source, first, Health(1));
        push_row(&mut source, second, Health(2));
        push_row(&mut source, third, Health(3));

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the lengths of 3, 2 and 1, and the target has no new columns
        let filled = unsafe {
            [
                source.move_entity_dropping(0, &mut target),
                source.move_entity_dropping(0, &mut target),
                source.move_entity_dropping(0, &mut target),
            ]
        };

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
    fn move_entity_dropping_with_several_columns_keeps_rows_together() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let name = ComponentInfo::of::<String>(ComponentMask::single(1));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(2));

        let mut source = Archetype::new(&[position, name]);
        let mut target = Archetype::new(&[name, health, position]);

        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);

        for (entity, x, label) in [(first, 1.0, "Ada"), (second, 2.0, "Bo"), (third, 3.0, "Cy")] {
            push_component(&mut source, Position(x));
            push_component(&mut source, String::from(label));

            // SAFETY: `push_entity` requires one new component in every column. The two lines above
            // push one onto `Position` and one onto `String`, which are the only columns
            unsafe { source.push_entity(entity) };
        }

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 3, and the line below pushes onto `Health`
        let filled = unsafe { source.move_entity_dropping(0, &mut target) };
        push_component(&mut target, Health(100));

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
    fn move_entity_dropping_with_no_columns_moves_only_entity() {
        let mut source = Archetype::new(&[]);
        let mut target = Archetype::new(&[]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);

        // SAFETY: `push_entity` requires one new component in every column, and the table has no
        // columns. `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 2, and the target has no new columns
        let filled = unsafe {
            source.push_entity(first);
            source.push_entity(second);
            source.move_entity_dropping(0, &mut target)
        };

        assert_eq!(filled, Some(second), "entity that fills the row");
        assert_eq!(source.entities, [second], "source entities");
        assert_eq!(target.entities, [first], "target entities");
    }

    /// Checks that a column of a type that takes no memory moves by count alone
    #[test]
    fn move_entity_dropping_with_zero_size_type_moves_count() {
        struct Tag;

        let tag = ComponentInfo::of::<Tag>(ComponentMask::single(0));
        let mut source = Archetype::new(&[tag]);
        let mut target = Archetype::new(&[tag]);
        push_row(&mut source, Entity::new(0, 0), Tag);
        push_row(&mut source, Entity::new(1, 0), Tag);

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 2, and the target has no new columns
        unsafe { source.move_entity_dropping(0, &mut target) };

        assert_eq!(read_column::<Tag>(&source).len(), 1, "source column length");
        assert_eq!(read_column::<Tag>(&target).len(), 1, "target column length");
    }

    /// Checks that a move passes ownership on, meaning two tables never clean up the same component
    #[test]
    fn move_entity_dropping_with_drop_counters_drops_each_component_once() {
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

        // SAFETY: `move_entity_dropping` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 3, and the target has no new columns
        unsafe { source.move_entity_dropping(0, &mut target) };
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
    fn move_entity_dropping_with_out_of_range_row_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        // SAFETY: this call breaks the row requirement on purpose. The `assert` at the start of
        // `move_entity_dropping` panics before any memory is read
        unsafe { source.move_entity_dropping(1, &mut target) };
    }

    /// Checks that an entity list shorter than its columns panics in every build
    #[test]
    #[should_panic]
    fn move_entity_dropping_with_short_entity_list_panics() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_component(&mut source, Position(1.0));

        // SAFETY: this call breaks the row requirement on purpose, for the entity list alone. The
        // `assert` at the start of `move_entity_dropping` panics before any memory is read
        unsafe { source.move_entity_dropping(0, &mut target) };
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
}
