use std::any::TypeId;
use crate::archetype::Archetype;
use crate::component::ComponentInfo;
use crate::world::World;

pub trait Bundle: Send + Sync + 'static {
    // Describes each type in the bundle, asking the world for each type's bit
    fn infos(world: &mut World) -> Vec<ComponentInfo>;
    // Pushes each value onto the column for its type
    fn push_into(self, table: &mut Archetype);
}

