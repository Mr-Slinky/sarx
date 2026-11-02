//! An archetype, which stores every entity that has one exact set of component types.

use crate::Entity;
use crate::component::{ComponentInfo, ComponentVec};
use std::any::TypeId;

/// The table for one set of component types.
pub(crate) struct Archetype {
    components: Vec<ComponentVec>,
    entities: Vec<Entity>,
    mask: u64,
}

impl Archetype {
    pub(crate) fn new(infos: &[ComponentInfo]) -> Self {
        let mut components: Vec<ComponentVec> =
            infos.iter().map(|info| ComponentVec::new(*info)).collect();
        // Sort by type id to ensure find_index_of can binary search
        components.sort_by_key(|components| components.info().type_id());
        let mut mask = 0;
        for info in infos {
            assert_eq!(
                mask & info.bit(),
                0,
                "Two component types in one table share a mask bit"
            );
            mask = mask | info.bit();
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

    /// Returns the `ComponentInfo` for each component type in the table, sorted by `TypeId`.
    pub(crate) fn infos(&self) -> Vec<ComponentInfo> {
        self.components.iter().map(ComponentVec::info).collect()
    }

    /// Returns the components of type `type_id`, or `None` where the table has none.
    pub(crate) fn components_of(&self, type_id: TypeId) -> Option<&ComponentVec> {
        let index = self.find_index_of(type_id)?;
        Some(&self.components[index])
    }

    /// Returns the components of type `type_id` for mutation, or `None`
    pub(crate) fn mut_components_of(&mut self, type_id: TypeId) -> Option<&mut ComponentVec> {
        let index = self.find_index_of(type_id)?;
        Some(&mut self.components[index])
    }

    fn find_index_of(&self, type_id: TypeId) -> Option<usize> {
        self.components
            .binary_search_by_key(&type_id, |components| components.info().type_id())
            .ok()
    }

    /// Returns the mask of the table, with one set bit for each component type that it stores.
    pub(crate) fn mask(&self) -> u64 {
        self.mask
    }

    /// Returns `true` where the table stores every component type in `query_mask`.
    pub(crate) fn has_all(&self, query_mask: u64) -> bool {
        self.mask & query_mask == query_mask
    }

    pub(crate) unsafe fn push_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Moves the entity in `row` to a new row at the end of `target`. Pops and swaps afterwards
    pub(crate) unsafe fn move_entity(
        &mut self,
        row: usize,
        target: &mut Archetype,
    ) -> Option<Entity> {
        for components in &mut self.components {
            let type_id = components.info().type_id();
            let target_components = target
                .mut_components_of(type_id)
                .expect("the target archetype stores every component type of the source");
            unsafe { components.move_component(row, target_components) };
        }
        let entity = self.entities.swap_remove(row);
        target.entities.push(entity);
        self.entities.get(row).copied()
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
            .expect("the table stores the component type");
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
            .expect("the table stores the component type");
        column.as_slice::<T>()
    }

    /// Checks the starting state of a table, from no columns up to a bit at the top of the mask
    #[test]
    fn new_with_varying_infos_returns_empty_table_with_combined_mask() {
        let position = ComponentInfo::of::<Position>(0b01);
        let health = ComponentInfo::of::<Health>(0b10);
        let name = ComponentInfo::of::<String>(1 << 63);

        // Lists the entries, the number of columns they should give, and the mask they should give
        let cases = [
            ("no entries", vec![], 0, 0),
            ("one entry", vec![position], 1, 0b01),
            ("two entries", vec![position, health], 2, 0b11),
            ("highest bit", vec![position, name], 2, (1 << 63) | 0b01),
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
    #[should_panic(expected = "Two component types in one table share a mask bit")]
    fn new_with_shared_bit_panics() {
        let position = ComponentInfo::of::<Position>(0b01);
        let health = ComponentInfo::of::<Health>(0b01);

        Archetype::new(&[position, health]);
    }

    /// Checks that the entry order makes no difference, and that each bit stays with its type
    #[test]
    fn infos_with_any_entry_order_returns_each_type_and_bit_sorted_by_type_id() {
        let position = ComponentInfo::of::<Position>(0b001);
        let health = ComponentInfo::of::<Health>(0b010);
        let name = ComponentInfo::of::<String>(0b100);

        let mut expected = vec![
            (TypeId::of::<Position>(), 0b001),
            (TypeId::of::<Health>(), 0b010),
            (TypeId::of::<String>(), 0b100),
        ];
        expected.sort();

        let cases = [
            [position, health, name],
            [name, health, position],
            [health, name, position],
        ];

        for (case, infos) in cases.iter().enumerate() {
            let table = Archetype::new(infos);

            let found: Vec<(TypeId, u64)> = table
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
            let mut table = Archetype::new(&[ComponentInfo::of::<Health>(1)]);
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
            ComponentInfo::of::<u8>(1 << 0),
            ComponentInfo::of::<u16>(1 << 1),
            ComponentInfo::of::<u32>(1 << 2),
            ComponentInfo::of::<u64>(1 << 3),
            ComponentInfo::of::<String>(1 << 4),
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
        let position = ComponentInfo::of::<Position>(0b01);
        let health = ComponentInfo::of::<Health>(0b10);
        let mut table = Archetype::new(&[position, health]);

        let column = table
            .mut_components_of(position.type_id())
            .expect("the table stores `Position`");
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
        let mut table = Archetype::new(&[ComponentInfo::of::<Position>(1)]);

        let found = table.mut_components_of(TypeId::of::<Health>());

        assert!(found.is_none());
    }

    /// Checks the query masks that match a table, along with the ones that ask for too much
    #[test]
    fn has_all_with_varying_query_masks_returns_expected_answer() {
        let position = ComponentInfo::of::<Position>(0b0001);
        let health = ComponentInfo::of::<Health>(0b0100);
        let table = Archetype::new(&[position, health]);

        // Pairs each query mask with the answer that it should give
        let cases = [
            (0b0101, true),
            (0b0001, true),
            (0b0100, true),
            (0, true),
            (0b0010, false),
            (0b0111, false),
            (u64::MAX, false),
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

        let matches_zero = table.has_all(0);
        let matches_one = table.has_all(1);

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
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(1)]);

        for (row, entity) in entities.into_iter().enumerate() {
            push_row(&mut table, entity, Health(row as u32));
        }

        assert_eq!(table.entities, entities);
    }

    /// Checks that rows line up where the entity goes in before its component
    #[test]
    fn push_entity_with_component_pushed_afterwards_lines_up_rows() {
        let entity = Entity::new(0, 0);
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(1)]);

        // SAFETY: `push_entity` requires one new component in every column. The line below pushes
        // one onto `Health`, which is the only column
        unsafe { table.push_entity(entity) };
        push_component(&mut table, Health(100));

        assert_eq!(table.entities, [entity], "entity list");
        assert_eq!(read_column::<Health>(&table), [Health(100)], "column");
    }

    /// Checks that the last row fills the empty row, in the entity list and in the column
    #[test]
    fn move_entity_with_middle_row_fills_gap_with_last_row() {
        let position = ComponentInfo::of::<Position>(1);
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

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 1 is below the length of 4, and the target has no new columns
        let filled = unsafe { source.move_entity(1, &mut target) };

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

    /// Checks that moving the last row changes no other row, and so reports no entity to update
    #[test]
    fn move_entity_with_last_row_returns_none() {
        let cases = [1_u32, 2, 5];

        for count in cases {
            let position = ComponentInfo::of::<Position>(1);
            let mut source = Archetype::new(&[position]);
            let mut target = Archetype::new(&[position]);
            for index in 0..count {
                push_row(&mut source, Entity::new(index, 0), Position(index as f32));
            }
            let last = count - 1;

            // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
            // target. Each case moves its last row, and the target has no new columns
            let filled = unsafe { source.move_entity(last as usize, &mut target) };

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
        let position = ComponentInfo::of::<Position>(1);
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        let moved = Entity::new(0, 0);
        let resident = Entity::new(1, 0);
        push_row(&mut source, moved, Position(1.0));
        push_row(&mut target, resident, Position(2.0));

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 1, and the target has no new columns
        unsafe { source.move_entity(0, &mut target) };

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
        let health = ComponentInfo::of::<Health>(1);
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        // Fills the target column to its first capacity of 4 components
        for index in 0..4_u32 {
            push_row(&mut target, Entity::new(index, 0), Health(index));
        }
        let moved = Entity::new(4, 0);
        push_row(&mut source, moved, Health(4));

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 1, and the target has no new columns
        unsafe { source.move_entity(0, &mut target) };

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
        let health = ComponentInfo::of::<Health>(1);
        let mut source = Archetype::new(&[health]);
        let mut target = Archetype::new(&[health]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);
        let third = Entity::new(2, 0);
        push_row(&mut source, first, Health(1));
        push_row(&mut source, second, Health(2));
        push_row(&mut source, third, Health(3));

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the lengths of 3, 2 and 1, and the target has no new columns
        let filled = unsafe {
            [
                source.move_entity(0, &mut target),
                source.move_entity(0, &mut target),
                source.move_entity(0, &mut target),
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
    fn move_entity_with_several_columns_keeps_rows_together() {
        let position = ComponentInfo::of::<Position>(0b001);
        let name = ComponentInfo::of::<String>(0b010);
        let health = ComponentInfo::of::<Health>(0b100);

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

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 3, and the line below pushes onto `Health`
        let filled = unsafe { source.move_entity(0, &mut target) };
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
    fn move_entity_with_no_columns_moves_only_entity() {
        let mut source = Archetype::new(&[]);
        let mut target = Archetype::new(&[]);
        let first = Entity::new(0, 0);
        let second = Entity::new(1, 0);

        // SAFETY: `push_entity` requires one new component in every column, and the table has no
        // columns. `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 2, and the target has no new columns
        let filled = unsafe {
            source.push_entity(first);
            source.push_entity(second);
            source.move_entity(0, &mut target)
        };

        assert_eq!(filled, Some(second), "entity that fills the row");
        assert_eq!(source.entities, [second], "source entities");
        assert_eq!(target.entities, [first], "target entities");
    }

    /// Checks that a column of a type that takes no memory moves by count alone
    #[test]
    fn move_entity_with_zero_size_type_moves_count() {
        struct Tag;

        let tag = ComponentInfo::of::<Tag>(1);
        let mut source = Archetype::new(&[tag]);
        let mut target = Archetype::new(&[tag]);
        push_row(&mut source, Entity::new(0, 0), Tag);
        push_row(&mut source, Entity::new(1, 0), Tag);

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 2, and the target has no new columns
        unsafe { source.move_entity(0, &mut target) };

        assert_eq!(read_column::<Tag>(&source).len(), 1, "source column length");
        assert_eq!(read_column::<Tag>(&target).len(), 1, "target column length");
    }

    /// Checks that a move passes ownership on (two tables never clean up one component)
    #[test]
    fn move_entity_with_drop_counters_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let counter = ComponentInfo::of::<DropCounter>(1);
        let mut source = Archetype::new(&[counter]);
        let mut target = Archetype::new(&[counter]);
        for index in 0..3 {
            let value = DropCounter {
                drops: Arc::clone(&drops),
            };
            push_row(&mut source, Entity::new(index, 0), value);
        }

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 3, and the target has no new columns
        unsafe { source.move_entity(0, &mut target) };
        let drops_after_move = drops.load(Ordering::Relaxed);

        drop(source);
        drop(target);
        let drops_after_tables = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_move, 0, "drops caused by the move");
        assert_eq!(drops_after_tables, 3, "drops after both tables are gone");
    }

    /// Checks that a move to a table lacking a column panics, since the component has no column
    #[test]
    #[should_panic(expected = "the target archetype stores every component type of the source")]
    fn move_entity_with_missing_target_column_panics() {
        let position = ComponentInfo::of::<Position>(1);
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[ComponentInfo::of::<Health>(1)]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        // SAFETY: `move_entity` requires a row in range, and a push onto each new column in the
        // target. Row 0 is below the length of 1, and the panic comes before any use of `target`
        unsafe { source.move_entity(0, &mut target) };
    }

    /// Checks that a debug build catches a row past the end of the columns
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "the index must be below the length")]
    fn move_entity_with_out_of_range_row_panics_in_debug_build() {
        let position = ComponentInfo::of::<Position>(1);
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_row(&mut source, Entity::new(0, 0), Position(1.0));

        // SAFETY: this call breaks the row requirement on purpose. The `debug_assert` in
        // `move_component` panics before any memory is read
        unsafe { source.move_entity(1, &mut target) };
    }

    /// Checks that an entity list shorter than its columns panics in every build
    #[test]
    #[should_panic(expected = "swap_remove index")]
    fn move_entity_with_short_entity_list_panics() {
        let position = ComponentInfo::of::<Position>(1);
        let mut source = Archetype::new(&[position]);
        let mut target = Archetype::new(&[position]);
        push_component(&mut source, Position(1.0));

        // SAFETY: this call breaks the row requirement on purpose, for the entity list alone. Row 0
        // is below the length of the column, meaning the column move reads a live component
        unsafe { source.move_entity(0, &mut target) };
    }

    /// Checks that dropping a table runs the clean-up code once for each component it stores
    #[test]
    fn drop_with_varying_row_counts_drops_each_component_once() {
        let cases = [0_u32, 1, 5];

        for count in cases {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut table = Archetype::new(&[ComponentInfo::of::<DropCounter>(1)]);
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
        let mut table = Archetype::new(&[ComponentInfo::of::<u32>(1)]);
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
