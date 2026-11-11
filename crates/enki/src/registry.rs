use crate::mask::ComponentMask;
use std::any::TypeId;
use std::collections::HashMap;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

pub struct ComponentRegistry {
    bits: HashMap<TypeId, ComponentMask>,
}

impl ComponentRegistry {
    pub(crate) fn new() -> Self {
        Self {
            bits: HashMap::new(),
        }
    }

    pub(crate) fn find_or_assign_bit(&mut self, type_id: TypeId) -> ComponentMask {
        // Counts the bits handed out so far, which is also the position of the next free bit
        let next_position = self.bits.len();
        // Returns the type's bit, first giving it the next free bit where it has none yet
        *self.bits.entry(type_id).or_insert_with(|| {
            // Panics where every bit is taken, since a mask has no bit left for another type
            let capacity = ComponentMask::CAPACITY;
            assert!(
                next_position < capacity,
                "a world has at most {capacity} component types"
            );
            ComponentMask::single(next_position)
        })
    }

    pub(crate) fn find_bit(&self, type_id: TypeId) -> Option<ComponentMask> {
        self.bits.get(&type_id).copied()
    }

    pub(crate) fn count_types(&self) -> usize {
        self.bits.len()
    }
}
