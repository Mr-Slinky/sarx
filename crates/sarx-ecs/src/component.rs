use std::alloc::{self, Layout};
use std::any::TypeId;
use std::ptr::NonNull;
use std::{mem, ptr};

/// A vector of components of one type in a single archetype.
/// Each component is stored as bytes in a single block of memory
pub(crate) struct ComponentVec {
    info: ComponentInfo, // the type that the column stores
    data: NonNull<u8>,   // the start of the block
    len: usize,          // the number of "rows" in use
    capacity: usize,     // the number of rows that the column vector has room for
}

impl ComponentVec {
    pub(crate) fn new(info: ComponentInfo) -> Self {
        let alignment = info.layout.align(); // Some power of 2
        let data = NonNull::new(ptr::without_provenance_mut(alignment))
            .expect("an alignment can never be zero");

        let capacity = if info.layout.size() == 0 {
            usize::MAX
        } else {
            0
        };

        Self {
            info,
            data,
            len: 0,
            capacity,
        }
    }

    /// Returns the facts about the component type that it stores.
    pub(crate) fn info(&self) -> ComponentInfo {
        self.info
    }

    /// Returns a pointer to the start of the block, where row 0 begins.
    pub(crate) fn data(&self) -> NonNull<u8> {
        self.data
    }

    pub(crate) fn pointer_at(&self, row: usize) -> *mut u8 {
        let offset = row * self.info.layout.size();
        self.data.as_ptr().wrapping_add(offset)
    }

    /// Doubles the size of the vector, or gives space for 4 "rows" when the vector is size 0
    pub(crate) fn grow(&mut self) {
        let new_capacity = if self.capacity == 0 {
            4
        } else {
            self.capacity * 2
        };
        let new_layout = array_layout(self.info.layout, new_capacity);

        let new_data = if self.capacity == 0 {
            // SAFETY: `alloc` requires a layout with a size above zero. This branch runs only at a
            // capacity of 0. `new` gives a component type of size zero a capacity of `usize::MAX`
            // instead, meaning the type here takes at least one byte. `new_layout` therefore
            // takes at least four bytes, one for each of its four rows
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout = array_layout(self.info.layout, self.capacity);
            // SAFETY: `realloc` has four requirements, and each one holds here.
            //
            // - The global allocator allocated the block. The capacity is above zero, meaning an
            //   earlier call to `grow` allocated it.
            // - `old_layout` matches the layout of that allocation exactly. `array_layout` builds
            //   it from `self.capacity`, the capacity that the earlier call allocated.
            // - The new size is above zero. `grow` runs only once every row is in use. A type of
            //   size zero never fills its capacity of `usize::MAX`, meaning the type here takes at
            //   least one byte.
            // - The new size stays within `isize::MAX`. `array_layout` checks this, and panics
            //   where it fails
            unsafe { alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) }
        };

        self.data = NonNull::new(new_data).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
        self.capacity = new_capacity;
    }

    pub(crate) unsafe fn push(&mut self, value: *const u8) {
        // Increase size if no space remains
        if self.len == self.capacity {
            self.grow();
        }

        let end = self.pointer_at(self.len); // last address in the column
        unsafe { ptr::copy_nonoverlapping(value, end, self.info.layout.size()) }
        self.len += 1;
    }

    /// Moves the component in `row` to the end of `target`, then fills `row` with the
    /// last component in self.
    pub(crate) unsafe fn move_row(&mut self, row: usize, target: &mut ComponentVec) {
        unsafe {
            target.push(self.pointer_at(row));
        }

        let last = self.len - 1;
        if row != last {
            let size = self.info.layout.size();
            unsafe {
                ptr::copy_nonoverlapping(self.pointer_at(last), self.pointer_at(row), size);
            }
        }
        self.len = last;
    }

}

impl Drop for ComponentVec {
    /// Drops every component, then frees the whole block.
    fn drop(&mut self) {
        
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ComponentInfo {
    pub(crate) type_id: TypeId,
    pub(crate) layout: Layout,
    pub(crate) drop: Option<unsafe fn(*mut u8)>,
}

impl ComponentInfo {
    pub(crate) fn of<T: 'static>() -> Self {
        let drop = if mem::needs_drop::<T>() {
            Some(drop_value::<T> as unsafe fn(*mut u8))
        } else {
            None
        };

        Self {
            type_id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            drop,
        }
    }
}

// ------------------------------------ Helpers

/// Drops the `T` that `pointer` points at.
///
/// # Safety
///
/// The caller must guarantee two things:
///
/// - `pointer` points at a live `T`, at an address that is correctly aligned for `T`.
/// - Nothing uses or drops that `T` afterwards, since its clean-up code has already run.
unsafe fn drop_value<T>(pointer: *mut u8) {
    // SAFETY: `drop_in_place` requires a pointer to a live `T`, at an address that is correctly
    // aligned for `T`. The caller guarantees both, under the Safety section above. The caller also
    // guarantees that nothing uses the `T` afterwards, since its clean-up code runs here
    unsafe { pointer.cast::<T>().drop_in_place() }
}

/// Returns the layout of a block, with room for `count` components, each with the layout `item`.
fn array_layout(item: Layout, count: usize) -> Layout {
    let size = item
        .size()
        .checked_mul(count)
        .expect("the block cannot exceed isize::MAX bytes");
    Layout::from_size_align(size, item.align()).expect("the block cannot exceed isize::MAX bytes")
}
