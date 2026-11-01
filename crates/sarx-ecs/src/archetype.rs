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
