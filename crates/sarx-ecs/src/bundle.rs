use std::any::TypeId;
use std::mem;
use crate::archetype::Archetype;
use crate::component::ComponentInfo;
use crate::world::World;

pub trait Bundle: Send + Sync + 'static {
    // Describes each type in the bundle, asking the world for each type's bit
    fn infos(world: &mut World) -> Vec<ComponentInfo>;
    // Pushes each value onto the column for its type
    fn push_into(self, table: &mut Archetype);
}

fn push_or_replace<T: Send + Sync + 'static>(
    table: &mut Archetype,
    row: usize,
    value: T,
) -> Option<T> {
    // Finds the column for `T`, and panics where the table has none
    let column = table
        .mut_components_of(TypeId::of::<T>())
        .expect("the table should have a column for every type in the bundle");

    // Replaces the component at `row`, where the column already holds one there
    if column.len() > row {
        // Finds the component at `row`
        let slot = &mut column.as_mut_slice::<T>()[row];
        // Puts `value` in its place, and returns the old component for the caller to drop
        return Some(mem::replace(slot, value));
    }

    // Moves the component onto the end of the column
    column.push_typed(value);
    // Reports that no component was replaced
    None
}
