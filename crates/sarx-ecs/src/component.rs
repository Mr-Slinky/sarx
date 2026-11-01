//! Defines the vector that stores the components of one type.
//!
//! An archetype is one set of component types. Each archetype has a table, which stores its
//! entities. The table has one column for each component type. A [`ComponentVec`] is one such
//! column. An entity takes the same row in every column. Row 5 of every column, for example,
//! belongs to the same entity.
//!
//! A [`ComponentVec`] stores its components next to each other, in one block of memory. A table
//! built from these vectors therefore has column-major order. Column-major order means that
//! neighbouring components in a column are also neighbours in memory. The components of one row lie
//! in separate blocks, one block per column.
//!
//! ADR 0011 decides that a table learns its component types at run time. A generic `Vec<T>` needs
//! its type at compile time, meaning it cannot serve as a column. A [`ComponentVec`] therefore
//! stores its components as raw bytes.
//!
//! A [`ComponentInfo`] describes those bytes to the vector. It stores the layout of the component
//! type, which is its size and alignment. It also stores a function that runs the clean-up code for
//! that type.
//!
//! The compiler cannot check the type of those bytes. As a result, each `unsafe` method has a
//! Safety section that lists what its caller must guarantee. One of those guarantees is that the
//! bytes belong to the component type that the vector stores.
//!
//! # Pointer validity
//!
//! A caller reaches a component through the raw pointer that [`ComponentVec::data`] or
//! [`ComponentVec::ptr_at`] returns. That pointer stops being valid when the vector grows its
//! block, even where the block stays at the same address. Using the pointer after that point is
//! undefined behaviour.
//!
//! The vector grows where [`ComponentVec::push`] finds the block full.
//! [`ComponentVec::move_component`] pushes onto its target, meaning it can grow the target too.
//!
//! A raw pointer has no lifetime, meaning the compiler accepts code that keeps one across a push.
//! The caller MUST guarantee that it uses no pointer from before a push. To reach the component
//! again, the caller requests a new pointer by index, since components keep their indices when the
//! vector grows.
//!
//! # Examples
//!
//! The example moves one `Position` component from one vector to another, as the ECS does when an
//! entity changes table. Each vector drops its components and frees its block when it goes out of
//! scope.
//!
//! ```ignore
//! // Defines the component type that the two vectors store
//! #[derive(Debug, PartialEq)]
//! struct Position {
//!     x: f32,
//!     y: f32,
//! }
//!
//! // Creates two empty vectors for `Position` components
//! let info = ComponentInfo::of::<Position>();
//! let mut source = ComponentVec::new(info);
//! let mut target = ComponentVec::new(source.info());
//!
//! // Pushes three components, which the vector copies in as bytes and then owns. `ManuallyDrop`
//! // stops each original from being dropped at the end of its loop iteration
//! let positions = [
//!     Position { x: 1.0, y: 1.0 },
//!     Position { x: 2.0, y: 2.0 },
//!     Position { x: 3.0, y: 3.0 },
//! ];
//! for position in positions {
//!     let position = ManuallyDrop::new(position);
//!     let bytes = ptr::from_ref(&*position).cast::<u8>();
//!     unsafe { source.push(bytes) };
//! }
//!
//! // Moves the component at index 0 to `target`, and fills index 0 with the last component
//! unsafe { source.move_component(0, &mut target) };
//!
//! // Borrows the components, through `data` at index 0 of `source` and `ptr_at` elsewhere. The
//! // vectors keep ownership of them
//! let first = unsafe { source.data().cast::<Position>().as_ref() };
//! let second = unsafe { &*source.ptr_at(1).cast::<Position>() };
//! let moved = unsafe { &*target.ptr_at(0).cast::<Position>() };
//!
//! assert_eq!(*first, Position { x: 3.0, y: 3.0 });
//! assert_eq!(*second, Position { x: 2.0, y: 2.0 });
//! assert_eq!(*moved, Position { x: 1.0, y: 1.0 });
//! ```

use std::alloc::{self, Layout};
use std::any::TypeId;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::{mem, ptr, slice};
// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Stores components of one type as raw bytes, in one block of memory that grows as the vector
/// fills.
///
/// The component at index `n` starts `n` times the component size past the start of the block.
///
/// The vector owns its components. A caller moves a component in with [`push`](Self::push), and
/// moves it to another vector with [`move_component`](Self::move_component). Dropping the vector
/// drops every component left in it, then frees the block.
///
/// # Examples
///
/// The example creates a vector for `f32`, pushes one component, then borrows it.
///
/// ```ignore
/// // Creates an empty vector for `f32` components
/// let mut components = ComponentVec::new(ComponentInfo::of::<f32>());
///
/// // Pushes one component, which the vector copies in as bytes. `f32` is `Copy` and has no
/// // clean-up code, meaning `value` needs no `ManuallyDrop`
/// let value = 1.5_f32;
/// let bytes = ptr::from_ref(&value).cast::<u8>();
/// unsafe { components.push(bytes) };
///
/// // Borrows index 0 as an `f32`
/// let stored = unsafe { &*components.ptr_at(0).cast::<f32>() };
/// ```
pub(crate) struct ComponentVec {
    info: ComponentInfo, // describes the component type that the vector stores
    data: NonNull<u8>,   // points at the start of the block
    len: usize,          // counts the components in the vector
    capacity: usize,     // counts the components that the block has room for
}

impl ComponentVec {
    /// Returns an empty vector for the component type that `info` describes.
    ///
    /// The first call to [`grow`](Self::grow) allocates the block. [`push`](Self::push) makes that
    /// call itself where the vector has no block yet. A vector for a type of size zero starts with
    /// a capacity of `usize::MAX`, and never allocates.
    pub(crate) fn new(info: ComponentInfo) -> Self {
        // Points `data` at an aligned address, which stands in until `grow` allocates a block
        let alignment = info.layout.align();
        let data = NonNull::new(ptr::without_provenance_mut(alignment))
            .expect("an alignment can never be zero");

        // Gives a type of size zero a capacity that `push` never reaches
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

    /// Returns the [`ComponentInfo`] that describes the component type in the vector.
    pub(crate) fn info(&self) -> ComponentInfo {
        self.info
    }

    /// Returns a pointer to the start of the block, where index 0 begins.
    ///
    /// This method is effectively the same as calling [`ptr_at`](Self::ptr_at) with an index of 0,
    /// which returns the same address. This method returns that address as a `NonNull`, a pointer
    /// type that is never null.
    ///
    /// The pointer is always aligned for the component type. Until the vector allocates a block, no
    /// memory lies behind it. The pointer stops being valid when [`grow`](Self::grow) runs.
    pub(crate) fn data(&self) -> NonNull<u8> {
        self.data
    }

    /// Returns a pointer to the start of the component at `index`.
    ///
    /// With an `index` of 0, this method returns the same address as [`data`](Self::data), which
    /// returns that address as a `NonNull`.
    ///
    /// The method performs no bounds check, meaning the pointer can lie past the end of the block.
    /// A caller reads through the pointer only where `index` is below the length. The pointer stops
    /// being valid when [`grow`](Self::grow) runs.
    ///
    /// A caller looks at a component by borrowing it through the pointer. The pointer method `read`
    /// makes a bitwise copy, and the vector still owns the original. Where the type has clean-up
    /// code and the copy gets dropped, the vector later drops the original as well, which is
    /// undefined behaviour.
    pub(crate) fn ptr_at(&self, index: usize) -> *mut u8 {
        let offset = index * self.info.layout.size();
        self.data.as_ptr().wrapping_add(offset)
    }

    /// Doubles the capacity, or sets it to 4 components where the capacity is 0.
    ///
    /// The components keep their indices. Every pointer that [`data`](Self::data) and
    /// [`ptr_at`](Self::ptr_at) returned earlier stops being valid, even where the block stays at
    /// the same address. Using one of those pointers afterwards is undefined behaviour.
    ///
    /// # Panics
    ///
    /// Panics where the component type has a size of zero, or where the new block would exceed
    /// `isize::MAX` bytes. The program aborts where the allocator cannot supply the block.
    fn grow(&mut self) {
        // Keeps a type of size zero away from the allocator, which requires a size above zero
        assert_ne!(
            self.info.layout.size(),
            0,
            "a component type of size zero never needs a block"
        );

        let new_capacity = if self.capacity == 0 {
            4
        } else {
            self.capacity * 2
        };
        let new_layout = array_layout(self.info.layout, new_capacity);

        let new_data = if self.capacity == 0 {
            // SAFETY: `alloc` requires a layout with a size above zero. The assert above means the
            // type takes at least one byte. `new_layout` therefore takes at least four bytes, one
            // for each of its four components
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout = array_layout(self.info.layout, self.capacity);
            // SAFETY: `realloc` has four requirements, and the call meets each one. The assert above
            // means the type takes at least one byte.
            //
            // - The global allocator allocated the block. The capacity is above zero, meaning an
            //   earlier call to `grow` allocated it.
            // - `old_layout` matches the layout of that allocation exactly. `array_layout` builds
            //   it from `self.capacity`, the capacity that the earlier call allocated.
            // - The new size is above zero, since the type takes at least one byte.
            // - The new size stays within `isize::MAX`. `array_layout` checks this, and panics
            //   where it fails
            unsafe { alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) }
        };

        self.data = NonNull::new(new_data).unwrap_or_else(|| alloc::handle_alloc_error(new_layout));
        self.capacity = new_capacity;
    }

    /// Copies one component from `value` onto the end of the vector.
    ///
    /// The vector grows its block where the block is full. It then owns the component.
    ///
    /// # Panics
    ///
    /// Panics where the vector is full and a larger block would exceed `isize::MAX` bytes. A vector
    /// for a type of size zero panics once it stores `usize::MAX` components. The program aborts
    /// where the allocator cannot supply the larger block.
    ///
    /// # Safety
    ///
    /// The caller must guarantee three things:
    ///
    /// - `value` points at a live component of the type that the vector stores.
    /// - `value` points outside the block of this vector, since growing can free the block.
    /// - Nothing drops the original afterwards, and nothing uses it unless its type is `Copy`.
    pub(crate) unsafe fn push(&mut self, value: *const u8) {
        if self.len == self.capacity {
            self.grow();
        }

        let end = self.ptr_at(self.len); // points just past the last component
        // SAFETY: `copy_nonoverlapping` requires two valid ranges that do not overlap. The caller
        // guarantees that `value` is valid and lies outside the block. `end` has room for one
        // component, since the check above grows a full block. For a type of size zero, the copy
        // covers zero bytes and touches no memory
        unsafe { ptr::copy_nonoverlapping(value, end, self.info.layout.size()) }
        self.len += 1;
    }

    pub(crate) fn push_typed<T: 'static>(&mut self, value: T) {
        assert_eq!(
            self.info.type_id,
            TypeId::of::<T>(),
            "the vector stores a different component type"
        );

        let value = ManuallyDrop::new(value);
        let bytes = ptr::from_ref(&*value).cast::<u8>();
        // SAFETY: `push` requires a live component of the stored type (outside the block) that nothing
        // drops afterwards. The assert confirms the type. `value` lies on the stack, and
        // `ManuallyDrop` stops it from being dropped here
        unsafe { self.push(bytes) }
    }

    /// Returns every component in the vector as a slice of `T`
    pub(crate) fn as_slice<T: 'static>(&self) -> &[T] {
        assert_eq!(
            self.info.type_id,
            TypeId::of::<T>(),
            "the vector stores a different component type"
        );

        // SAFETY: `from_raw_parts` requires an aligned pointer to `len` live values of `T`. The assert
        // confirms the type, `data` is always aligned, and every index below `len` stores a live
        // component. An empty vector gives an empty slice, which reads no memory
        unsafe { slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Returns every component in the vector as a slice of `T`, for changing.
    pub(crate) fn as_mut_slice<T: 'static>(&mut self) -> &mut [T] {
        assert_eq!(
            self.info.type_id,
            TypeId::of::<T>(),
            "the vector stores a different component type"
        );

        // SAFETY: `from_raw_parts_mut` has the same requirements as `from_raw_parts` in `as_slice`,
        // which hold here for the same reasons. It also requires that nothing else accesses those
        // components while the slice exists, and the borrow of `&mut self` guarantees that
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Moves the component at `index` onto the end of `target`, then fills `index` with the last
    /// component in `self`.
    ///
    /// `self` ends up one component shorter. `target` owns the moved component. The component that
    /// was last now lives at `index`, meaning the caller updates the recorded location of its
    /// entity. Where `index` is the last index, no other component moves, and the caller updates no
    /// other entity.
    ///
    /// # Panics
    ///
    /// Panics where [`push`](Self::push) panics on `target`. The program aborts where the allocator
    /// cannot supply a larger block for `target`.
    ///
    /// # Safety
    ///
    /// The caller must guarantee two things:
    ///
    /// - `index` is below the length of `self`.
    /// - `target` stores the same component type as `self`.
    ///
    /// A debug build panics where either guarantee fails. A release build skips both checks.
    pub(crate) unsafe fn move_component(&mut self, index: usize, target: &mut ComponentVec) {
        debug_assert!(index < self.len, "the index must be below the length");
        debug_assert_eq!(
            self.info.type_id, target.info.type_id,
            "both vectors must store the same component type"
        );

        // SAFETY: `push` requires a live component of the type in `target`, and the caller
        // guarantees both. Both references are exclusive, which makes `target` a separate vector.
        // As a result, the pointer lies outside its block. The fill below overwrites the original,
        // or the shorter length leaves it out of use. Nothing therefore uses or drops it
        // afterwards
        unsafe {
            target.push(self.ptr_at(index));
        }

        let last = self.len - 1;
        if index != last {
            let size = self.info.layout.size();
            // SAFETY: `copy_nonoverlapping` requires two valid ranges that do not overlap. `index`
            // and `last` are different indices below the length, meaning each range lies inside
            // the block, apart from the other. For a type of size zero, each range covers zero
            // bytes
            unsafe {
                ptr::copy_nonoverlapping(self.ptr_at(last), self.ptr_at(index), size);
            }
        }
        self.len = last;
    }
}

impl Drop for ComponentVec {
    /// Drops every component in the vector, then frees the block.
    fn drop(&mut self) {
        if let Some(drop_value) = self.info.drop {
            for index in 0..self.len {
                // SAFETY: `drop_value` requires a live, aligned component that nothing uses
                // afterwards. Every index below `len` stores a live component. Each address is
                // aligned for the type, since `data` is aligned and the component size is a
                // multiple of the alignment. `drop` runs once, and no code can use the vector after
                // it
                unsafe {
                    drop_value(self.ptr_at(index));
                }
            }
        }

        // Frees the block, which exists only for a type above size zero and a capacity above zero
        let layout_non_empty = self.info.layout.size() != 0;
        let cap_non_empty = self.capacity != 0;
        if layout_non_empty && cap_non_empty {
            let layout = array_layout(self.info.layout, self.capacity);
            // SAFETY: `dealloc` requires a block from the global allocator, along with the layout
            // that the allocation used. The check above means `grow` allocated the block.
            // `array_layout` builds the same layout from the same capacity
            unsafe {
                alloc::dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}

// SAFETY: `ComponentInfo::of` is the only way to build a `ComponentInfo`, and it requires
// `T: Send + Sync`. Every component in the vector can therefore move between threads, and be read
// from several threads at the same time. Changing a component or the vector requires `&mut`, or an
// `unsafe` call whose caller MUST guarantee that nothing else accesses that component
unsafe impl Send for ComponentVec {}
// SAFETY: as for `Send` above
unsafe impl Sync for ComponentVec {}

/// Describes a component type to code that knows the type only at run time.
///
/// # Examples
///
/// The example builds the `ComponentInfo` for `f32`.
///
/// ```ignore
/// // Builds the `ComponentInfo` for `f32`, a type with no clean-up code
/// let info = ComponentInfo::of::<f32>();
/// ```
#[derive(Clone, Copy, Debug)]
pub(crate) struct ComponentInfo {
    /// Identifies the component type.
    type_id: TypeId,
    /// Defines the size and alignment of one component.
    layout: Layout,
    /// Stores the function that runs the clean-up code of the component at the given address. The
    /// field is `None` where the type has no clean-up code.
    drop: Option<unsafe fn(*mut u8)>,
}

impl ComponentInfo {
    /// Returns the [`ComponentInfo`] for the component type `T`.
    ///
    /// The bound `T: 'static` means that `T` contains no borrowed references that could expire. A
    /// vector can then keep a `T` for as long as it needs. `TypeId::of` requires the same bound.
    pub(crate) fn of<T: Send + Sync + 'static>() -> Self {
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

    pub(crate) fn type_id(self) -> TypeId {
        self.type_id
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

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
///
/// # Panics
///
/// Panics where the block would exceed `isize::MAX` bytes.
fn array_layout(item: Layout, count: usize) -> Layout {
    let size = item
        .size()
        .checked_mul(count)
        .expect("the block cannot exceed isize::MAX bytes");
    Layout::from_size_align(size, item.align()).expect("the block cannot exceed isize::MAX bytes")
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

    /// Adds 1 to a shared counter when dropped, which shows how many times clean-up code ran.
    struct DropCounter {
        drops: Arc<AtomicUsize>,
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Checks that size, alignment and clean-up are recorded separately, since each can vary alone
    #[test]
    fn of_with_standard_and_extreme_types_returns_layout_and_drop_function() {
        // Defines a type of size zero that has clean-up code
        #[allow(dead_code)]
        struct DropOnly;
        impl Drop for DropOnly {
            fn drop(&mut self) {}
        }

        // Defines a type with a larger alignment than its one byte of data needs
        #[allow(dead_code)]
        #[repr(align(64))]
        struct Aligned(u8);

        // Builds one description per type, beside the size, alignment and clean-up it should report
        let cases = [
            ("f32", ComponentInfo::of::<f32>(), 4, 4, false),
            (
                "String",
                ComponentInfo::of::<String>(),
                size_of::<String>(),
                align_of::<String>(),
                true,
            ),
            ("()", ComponentInfo::of::<()>(), 0, 1, false),
            ("DropOnly", ComponentInfo::of::<DropOnly>(), 0, 1, true),
            ("Aligned", ComponentInfo::of::<Aligned>(), 64, 64, false),
        ];

        for (name, info, size, alignment, has_drop) in cases {
            assert_eq!(info.layout.size(), size, "size of {name}");
            assert_eq!(info.layout.align(), alignment, "alignment of {name}");
            assert_eq!(info.drop.is_some(), has_drop, "drop function of {name}");
        }
    }

    /// Checks that a vector can tell two types apart when their bytes look identical
    #[test]
    fn of_with_same_layout_types_returns_different_type_ids() {
        // Defines two separate types with identical fields
        #[allow(dead_code)]
        struct Health(u32);
        #[allow(dead_code)]
        struct Armour(u32);

        let health = ComponentInfo::of::<Health>();
        let armour = ComponentInfo::of::<Armour>();

        assert_eq!(health.layout, armour.layout, "layouts should match");
        assert_ne!(health.type_id, armour.type_id, "type IDs should differ");
    }

    /// Checks the starting state that a vector has before it allocates any memory
    #[test]
    fn new_with_standard_types_returns_empty_vector_with_aligned_pointer() {
        let cases = [
            ("u8", ComponentInfo::of::<u8>()),
            ("f32", ComponentInfo::of::<f32>()),
            ("u64", ComponentInfo::of::<u64>()),
        ];

        for (name, info) in cases {
            let components = ComponentVec::new(info);

            let address = components.data().as_ptr().addr();
            let remainder = address % info.layout.align();

            assert_eq!(components.len, 0, "length for {name}");
            assert_eq!(components.capacity, 0, "capacity for {name}");
            assert_eq!(remainder, 0, "pointer alignment for {name}");
        }
    }

    /// Checks that a vector with no block of memory can still be viewed safely
    #[test]
    fn as_slice_with_empty_vector_returns_empty_slice() {
        let components = ComponentVec::new(ComponentInfo::of::<f32>());
        let slice = components.as_slice::<f32>();

        assert!(slice.is_empty(), "slice should have no components");
    }

    /// Checks that a component survives being stored as bytes and read back as its own type
    #[test]
    fn push_typed_with_one_value_stores_value_for_as_slice() {
        let mut components = ComponentVec::new(ComponentInfo::of::<f32>());

        components.push_typed(1.5_f32);

        assert_eq!(components.as_slice::<f32>(), [1.5]);
    }

    /// Checks that the raw route hands ownership of a heap-owning component to the vector
    #[test]
    fn push_with_manually_dropped_string_stores_value_for_as_slice() {
        let mut components = ComponentVec::new(ComponentInfo::of::<String>());
        let value = ManuallyDrop::new(String::from("villager"));
        let bytes = ptr::from_ref(&*value).cast::<u8>();

        // SAFETY: `push` requires a live `String` outside the block that nothing drops afterwards.
        // `value` lies on the stack, and `ManuallyDrop` stops it from being dropped
        unsafe { components.push(bytes) };

        assert_eq!(components.as_slice::<String>(), ["villager"]);
    }

    /// Checks that components are spaced one component size apart, starting at `data`
    #[test]
    fn ptr_at_with_standard_and_extreme_indices_returns_offset_address() {
        let components = ComponentVec::new(ComponentInfo::of::<u32>());
        let start = components.data().as_ptr().addr();

        // Pairs each index with its expected distance in bytes from the start of the block
        let cases = [(0, 0), (1, 4), (5, 20)];

        for (index, offset) in cases {
            let address = components.ptr_at(index).addr();

            assert_eq!(address, start + offset, "address at index {index}");
        }
    }

    /// Checks that a change reaches the stored component, and leaves its neighbours alone
    #[test]
    fn as_mut_slice_with_changed_value_updates_only_that_component() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>());
        components.push_typed(1_u32);
        components.push_typed(2_u32);
        components.push_typed(3_u32);

        components.as_mut_slice::<u32>()[1] = 20;

        assert_eq!(components.as_slice::<u32>(), [1, 20, 3]);
    }

    /// Checks that the block starts at 4 components, then doubles only once it is full
    #[test]
    fn push_typed_with_increasing_counts_returns_expected_capacity() {
        // Pairs each number of pushes with the capacity that it should produce
        let cases = [(1_u32, 4), (4, 4), (5, 8), (8, 8), (9, 16)];

        for (count, capacity) in cases {
            let mut components = ComponentVec::new(ComponentInfo::of::<u32>());

            for value in 0..count {
                components.push_typed(value);
            }

            assert_eq!(
                components.capacity, capacity,
                "capacity after {count} pushes"
            );
        }
    }

    /// Checks that moving to a larger block keeps every component at its index
    #[test]
    fn push_typed_with_growth_keeps_values_in_order() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>());

        // Pushes 9 components, which makes the block grow from 4 to 8, then from 8 to 16
        for value in 0..9_u32 {
            components.push_typed(value);
        }

        assert_eq!(components.as_slice::<u32>(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    /// Checks that heap memory owned by a component stays reachable after its block moves
    #[test]
    fn push_typed_with_growth_keeps_strings_readable() {
        let names = ["Ada", "Bo", "Cy", "Dee", "Eli"];
        let mut components = ComponentVec::new(ComponentInfo::of::<String>());

        // Pushes 5 components, which makes the block grow from 4 to 8
        for name in names {
            components.push_typed(String::from(name));
        }

        assert_eq!(components.as_slice::<String>(), names);
    }

    /// Checks that dropping a vector runs the clean-up code once for each component it stores
    #[test]
    fn drop_with_varying_counts_drops_each_component_once() {
        let cases = [0, 1, 5];

        for count in cases {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut components = ComponentVec::new(ComponentInfo::of::<DropCounter>());
            for _ in 0..count {
                components.push_typed(DropCounter {
                    drops: Arc::clone(&drops),
                });
            }

            drop(components);

            assert_eq!(drops.load(Ordering::Relaxed), count, "drops for {count}");
        }
    }

    /// Checks that moving to a larger block leaves every component alive
    #[test]
    fn push_typed_with_growth_drops_no_components() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut components = ComponentVec::new(ComponentInfo::of::<DropCounter>());

        // Pushes 9 components, which makes the block grow from 4 to 8, then from 8 to 16
        for _ in 0..9 {
            components.push_typed(DropCounter {
                drops: Arc::clone(&drops),
            });
        }

        assert_eq!(drops.load(Ordering::Relaxed), 0, "drops while the vector is alive");
    }

    /// Checks the swap and pop: the middle component moves across, and the last one fills its gap
    #[test]
    fn move_component_with_middle_index_fills_gap_with_last_component() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>());
        source.push_typed(10_u32);
        source.push_typed(20_u32);
        source.push_typed(30_u32);
        source.push_typed(40_u32);

        let mut target = ComponentVec::new(source.info());

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 1 is below the length of 4, and `target` uses the same `ComponentInfo`
        unsafe { source.move_component(1, &mut target) };

        assert_eq!(source.as_slice::<u32>(), [10, 40, 30], "source after the move");
        assert_eq!(target.as_slice::<u32>(), [20], "target after the move");
    }

    /// Checks that moving the final component only pops it, since no gap is left to fill
    #[test]
    fn move_component_with_last_index_leaves_other_components_in_place() {
        // Lists the starting components, the index to move, and the components that should remain
        let cases: [(&[u32], usize, &[u32]); 3] = [
            (&[10, 20, 30], 2, &[10, 20]),
            (&[10], 0, &[]),
            (
                &[10, 20, 30, 40, 50, 60, 70, 80, 90],
                8,
                &[10, 20, 30, 40, 50, 60, 70, 80],
            ),
        ];

        for (values, index, remaining) in cases {
            let mut source = ComponentVec::new(ComponentInfo::of::<u32>());
            for &value in values {
                source.push_typed(value);
            }

            let mut target = ComponentVec::new(source.info());

            // SAFETY: `move_component` requires an index below the length and a target of the same
            // type. Each case gives its last index, and `target` uses the same `ComponentInfo`
            unsafe { source.move_component(index, &mut target) };

            assert_eq!(source.as_slice::<u32>(), remaining, "source for {values:?}");
            assert_eq!(target.as_slice::<u32>(), [values[index]], "target for {values:?}");
        }
    }

    /// Checks that a move into a full target grows the target and keeps what it already stored
    #[test]
    fn move_component_with_full_target_grows_target() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>());
        source.push_typed(50_u32);

        let mut target = ComponentVec::new(source.info());
        for value in [10_u32, 20, 30, 40] {
            target.push_typed(value);
        }

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 0 is below the length of 1, and `target` uses the same `ComponentInfo`
        unsafe { source.move_component(0, &mut target) };

        assert_eq!(target.capacity, 8, "target capacity after the move");
        assert_eq!(target.as_slice::<u32>(), [10, 20, 30, 40, 50], "target after the move");
    }

    /// Checks that a move passes ownership on (two vectors never clean up one component)
    #[test]
    fn move_component_with_drop_counters_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut source = ComponentVec::new(ComponentInfo::of::<DropCounter>());
        for _ in 0..5 {
            source.push_typed(DropCounter {
                drops: Arc::clone(&drops),
            });
        }

        let mut target = ComponentVec::new(source.info());

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 1 is below the length of 5, index 3 is below the length of 4, and `target`
        // uses the same `ComponentInfo`
        unsafe {
            source.move_component(1, &mut target); // fills the gap with the last component
            source.move_component(3, &mut target); // moves the last component, leaving no gap
        }
        let drops_after_moves = drops.load(Ordering::Relaxed);

        drop(source);
        drop(target);
        let drops_after_vectors = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_moves, 0, "drops caused by the moves");
        assert_eq!(drops_after_vectors, 5, "drops after both vectors are gone");
    }

    /// Checks that a type taking no memory is still counted, and never needs a block
    #[test]
    fn push_typed_with_zero_size_type_counts_without_allocating() {
        let mut components = ComponentVec::new(ComponentInfo::of::<()>());
        let start = components.data();

        for _ in 0..5 {
            components.push_typed(());
        }

        assert_eq!(components.as_slice::<()>().len(), 5, "number of components");
        assert_eq!(components.capacity, usize::MAX, "capacity");
        assert_eq!(components.data(), start, "pointer to the block");
    }

    /// Checks that a type taking no memory still has its clean-up code run for each component
    #[test]
    fn drop_with_zero_size_drop_type_drops_each_component_once() {
        // Counts the drops outside the type, since a type of size zero has no room for a counter
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Marker;
        impl Drop for Marker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut components = ComponentVec::new(ComponentInfo::of::<Marker>());
        for _ in 0..3 {
            components.push_typed(Marker);
        }

        drop(components);

        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    /// Checks that a vector of a type that takes no memory stops at its limit, rather than allocating
    #[test]
    #[should_panic(expected = "a component type of size zero never needs a block")]
    fn push_typed_with_full_zero_size_vector_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<()>());

        // Sets the length by hand, since pushing `usize::MAX` components would take too long
        components.len = usize::MAX;

        components.push_typed(());
    }

    /// Checks that a type with a large alignment stays aligned at every index as the block grows
    #[test]
    fn push_typed_with_large_alignment_keeps_every_component_aligned() {
        #[repr(align(64))]
        struct Aligned(u8);

        let mut components = ComponentVec::new(ComponentInfo::of::<Aligned>());

        // Pushes 9 components, which makes the block grow from 4 to 8, then from 8 to 16
        for value in 0..9_u8 {
            components.push_typed(Aligned(value));
        }

        let remainders: Vec<usize> = (0..9)
            .map(|index| components.ptr_at(index).addr() % 64)
            .collect();
        let values: Vec<u8> = components
            .as_slice::<Aligned>()
            .iter()
            .map(|component| component.0)
            .collect();

        assert_eq!(remainders, [0; 9], "remainder of each address divided by 64");
        assert_eq!(values, [0, 1, 2, 3, 4, 5, 6, 7, 8], "stored values");
    }

    /// Checks that a value of another type cannot enter the vector, even at the same size
    #[test]
    #[should_panic(expected = "the vector stores a different component type")]
    fn push_typed_with_wrong_type_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>());

        components.push_typed(1.5_f32);
    }

    /// Checks that stored bytes cannot be read as another type, even at the same size
    #[test]
    #[should_panic(expected = "the vector stores a different component type")]
    fn as_slice_with_wrong_type_panics() {
        let components = ComponentVec::new(ComponentInfo::of::<u32>());

        components.as_slice::<f32>();
    }

    /// Checks that stored bytes cannot be changed as another type, even at the same size
    #[test]
    #[should_panic(expected = "the vector stores a different component type")]
    fn as_mut_slice_with_wrong_type_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>());

        components.as_mut_slice::<f32>();
    }

    /// Checks that a debug build catches an index that would read past the last component
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "the index must be below the length")]
    fn move_component_with_out_of_range_index_panics_in_debug_build() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>());
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(source.info());

        // SAFETY: this call breaks the index requirement on purpose. The `debug_assert` in
        // `move_component` panics before any memory is read
        unsafe { source.move_component(1, &mut target) };
    }

    /// Checks that a debug build catches a move between columns of different types
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "both vectors must store the same component type")]
    fn move_component_with_mismatched_target_panics_in_debug_build() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>());
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(ComponentInfo::of::<f32>());

        // SAFETY: this call breaks the type requirement on purpose. The `debug_assert_eq` in
        // `move_component` panics before any memory is read
        unsafe { source.move_component(0, &mut target) };
    }

    /// Checks the size calculation for an ordinary block, and for the largest block allowed
    #[test]
    fn array_layout_with_standard_and_extreme_counts_returns_block_layout() {
        let limit = isize::MAX as usize;

        // Lists the component layout, the count, and the block size and alignment it should give
        let cases = [
            (Layout::new::<u32>(), 3, 12, 4),
            (Layout::new::<u8>(), limit, limit, 1),
        ];

        for (item, count, size, alignment) in cases {
            let layout = array_layout(item, count);

            assert_eq!(layout.size(), size, "size for count {count}");
            assert_eq!(layout.align(), alignment, "alignment for count {count}");
        }
    }

    /// Checks the limit when the block size is too large for `usize` to represent
    #[test]
    #[should_panic(expected = "the block cannot exceed isize::MAX bytes")]
    fn array_layout_with_overflowing_size_panics() {
        array_layout(Layout::new::<u64>(), usize::MAX);
    }

    /// Checks the limit when the block size fits in `usize`, but passes the largest size allowed
    #[test]
    #[should_panic(expected = "the block cannot exceed isize::MAX bytes")]
    fn array_layout_with_size_above_limit_panics() {
        let count = isize::MAX as usize + 1;

        array_layout(Layout::new::<u8>(), count);
    }

    /// Checks that threads can read one vector together, and that a vector can move to a thread
    #[test]
    fn as_slice_with_several_threads_returns_same_components() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>());
        for value in [1_u32, 2, 3, 4] {
            components.push_typed(value);
        }

        // Adds up the components, borrowing the vector rather than taking ownership of it
        let read_total = || components.as_slice::<u32>().iter().sum::<u32>();

        let shared_totals = thread::scope(|scope| {
            let handles = [scope.spawn(read_total), scope.spawn(read_total)];
            handles.map(|handle| handle.join().unwrap())
        });

        let moved_total = thread::spawn(move || components.as_slice::<u32>().iter().sum::<u32>())
            .join()
            .unwrap();

        assert_eq!(shared_totals, [10, 10], "totals from the shared vector");
        assert_eq!(moved_total, 10, "total from the moved vector");
    }

    /// Checks that a whole entity can change table, with its columns staying in step row by row
    #[test]
    fn move_component_with_two_columns_keeps_rows_together() {
        let mut source_health = ComponentVec::new(ComponentInfo::of::<u32>());
        let mut source_names = ComponentVec::new(ComponentInfo::of::<String>());
        for (health, name) in [(10_u32, "Ada"), (20, "Bo"), (30, "Cy")] {
            source_health.push_typed(health);
            source_names.push_typed(String::from(name));
        }

        let mut target_health = ComponentVec::new(source_health.info());
        let mut target_names = ComponentVec::new(source_names.info());

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 0 is below the length of 3, and each target uses its source's `ComponentInfo`
        unsafe {
            source_health.move_component(0, &mut target_health);
            source_names.move_component(0, &mut target_names);
        }

        assert_eq!(source_health.as_slice::<u32>(), [30, 20], "source health");
        assert_eq!(source_names.as_slice::<String>(), ["Cy", "Bo"], "source names");
        assert_eq!(target_health.as_slice::<u32>(), [10], "target health");
        assert_eq!(target_names.as_slice::<String>(), ["Ada"], "target names");
    }
}
