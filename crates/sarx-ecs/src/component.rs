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
use std::ptr::NonNull;
use std::{mem, ptr};

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
    pub(crate) type_id: TypeId,
    /// Defines the size and alignment of one component.
    pub(crate) layout: Layout,
    /// Stores the function that runs the clean-up code of the component at the given address. The
    /// field is `None` where the type has no clean-up code.
    pub(crate) drop: Option<unsafe fn(*mut u8)>,
}

impl ComponentInfo {
    /// Returns the [`ComponentInfo`] for the component type `T`.
    ///
    /// The bound `T: 'static` means that `T` contains no borrowed references that could expire. A
    /// vector can then keep a `T` for as long as it needs. `TypeId::of` requires the same bound.
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
