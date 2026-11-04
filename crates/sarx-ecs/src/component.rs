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
//! A table learns its component types at run time, as ADR 0011 sets out. A generic `Vec<T>` needs
//! its type at compile time, meaning it cannot serve as a column. A [`ComponentVec`] therefore
//! stores its components as raw bytes.
//!
//! A [`ComponentInfo`] describes those bytes to the vector. It stores the layout of the component
//! type, which is its size and alignment. It also stores a function that runs the clean-up code for
//! that type. The identity and name of the type are stored there as well, for the typed methods to
//! check.
//!
//! A [`ComponentInfo`] also stores one bit for the table that the column belongs to. A table has a
//! mask, which is a `u64` with one set bit for each component type that the table stores. The table
//! builds its mask from these bits. A [`ComponentVec`] never reads the bit.
//!
//! The compiler cannot check the type of those bytes. As a result, each `unsafe` method has a
//! Safety section that lists what its caller MUST guarantee. One of those guarantees is that the
//! bytes belong to the component type that the vector stores.
//!
//! # Typed and raw access
//!
//! A caller reaches the components in one of two ways. The choice depends on whether the caller
//! knows the component type at compile time.
//!
//! A caller that knows the type uses [`ComponentVec::as_slice`] or [`ComponentVec::as_mut_slice`].
//! Each method panics where that type differs from the type that the vector stores. Indexing the
//! returned slice panics where the index is out of range. The slice also borrows the vector,
//! meaning the compiler rejects a push while the slice is in use. This is the safe way to reach a
//! component.
//!
//! A caller that has only a [`ComponentInfo`], such as a table that learns its column types at run
//! time, cannot name the type that those methods require. That caller uses
//! [`ComponentVec::data`] or [`ComponentVec::ptr_at`], which return a raw pointer. Neither method
//! checks the type or the index. Reading through the pointer requires `unsafe`, and the caller MUST
//! guarantee that the type, the index and the pointer are all valid. A caller that writes through
//! the pointer MUST also guarantee that nothing else accesses that component.
//!
//! # Pointer validity
//!
//! The raw pointer that [`ComponentVec::data`] or [`ComponentVec::ptr_at`] returns stops being
//! valid when the vector grows its block, even where the block stays at the same address. Using the
//! pointer after that point is undefined behaviour.
//!
//! The vector grows where [`ComponentVec::push_typed`] finds the block full.
//! [`ComponentVec::move_component`] pushes onto its target, meaning it can grow the target too.
//!
//! Dropping the vector frees its block. Every pointer into that block then stops being valid, and
//! using one is undefined behaviour.
//!
//! [`ComponentVec::move_component`] also changes what the addresses in `self` contain, although it
//! frees nothing. The address of the old last component now contains a stale copy of that
//! component. Where a caller drops the component through that address, the component gets dropped
//! twice, which is undefined behaviour. Where the moved index is not the last index, a pointer to it
//! now reaches a different component, namely the one that was last.
//!
//! A raw pointer has no lifetime, meaning the compiler accepts code that keeps one across a push, a
//! move or the drop of the vector. A push can grow the block. The caller therefore MUST guarantee
//! that it does not use any pointer from before a push, a move or the drop. To reach a component
//! again, the caller requests a new pointer by index. Components keep their indices when the
//! vector grows. After a move from an index other than the last, the component that was last is
//! stored at the moved index.
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
//! // Creates two empty vectors for `Position` components, with the mask value `1` for that type
//! let info = ComponentInfo::of::<Position>(1);
//! let mut source = ComponentVec::new(info);
//! let mut target = ComponentVec::new(source.info());
//!
//! // Pushes three components, which the vector then owns
//! let positions = [
//!     Position { x: 1.0, y: 1.0 },
//!     Position { x: 2.0, y: 2.0 },
//!     Position { x: 3.0, y: 3.0 },
//! ];
//! for position in positions {
//!     source.push_typed(position);
//! }
//!
//! // Moves the component at index 0 to `target`, and fills index 0 with the last component
//! // SAFETY: `move_component` requires an index below the length and a target of the same type.
//! // Index 0 is below the length of 3, and `target` uses the same `ComponentInfo`
//! unsafe { source.move_component(0, &mut target) };
//!
//! // Borrows the components as slices of `Position`. The vectors keep ownership of them
//! let remaining = source.as_slice::<Position>();
//! let moved = target.as_slice::<Position>();
//!
//! assert_eq!(remaining[0], Position { x: 3.0, y: 3.0 });
//! assert_eq!(remaining[1], Position { x: 2.0, y: 2.0 });
//! assert_eq!(moved[0], Position { x: 1.0, y: 1.0 });
//! ```

use std::alloc::{self, Layout};
use std::any::{self, TypeId};
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
/// The vector owns its components. A caller moves a component in with
/// [`push_typed`](Self::push_typed), and moves it to another vector with
/// [`move_component`](Self::move_component). Dropping the vector drops every component left in it,
/// then frees the block.
///
/// # Examples
///
/// The example creates a vector for `f32`, pushes one component, borrows it, then changes it.
///
/// ```ignore
/// // Creates an empty vector for `f32` components, with the mask value `1` for that type
/// let mut components = ComponentVec::new(ComponentInfo::of::<f32>(1));
///
/// // Pushes one component, which the vector then owns
/// components.push_typed(1.5_f32);
///
/// // Borrows index 0 as an `f32`. `as_slice` checks the type, and the slice checks the index
/// let stored = &components.as_slice::<f32>()[0];
/// assert_eq!(*stored, 1.5);
///
/// // Changes index 0, through a slice that allows changes
/// components.as_mut_slice::<f32>()[0] = 2.5;
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
    /// The first call to [`grow`](Self::grow) allocates the block. A push, or a move into this
    /// vector, leads to that call where the vector has no block yet. A vector for a type of size
    /// zero starts with a capacity of `usize::MAX`, and never allocates.
    pub(crate) fn new(info: ComponentInfo) -> Self {
        // Points `data` at an aligned address, which stands in until `grow` allocates a block
        let alignment = info.layout.align();
        let data = NonNull::new(ptr::without_provenance_mut(alignment))
            .expect("the alignment should be above zero, which `Layout` guarantees");

        // Gives a type of size zero a capacity of `usize::MAX`, which keeps `push` away from `grow`
        // until that limit
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
    /// This method returns the same address as [`ptr_at`](Self::ptr_at) with an index of 0, as a
    /// `NonNull`, a pointer type that is never null.
    ///
    /// The pointer is always aligned for the component type. Until the vector allocates a block, the
    /// pointer does not point at any memory. The pointer stops being valid when the vector grows its block or is
    /// dropped. The Pointer validity section in the module documentation sets out every rule.
    pub(crate) fn data(&self) -> NonNull<u8> {
        self.data
    }

    /// Returns a pointer to the start of the component at `index`.
    ///
    /// The method does not check bounds, meaning the pointer can lie past the end of the block.
    /// To read through the pointer, the caller MUST guarantee that `index` is below the length. The
    /// pointer stops being valid when the vector grows its block or is dropped. The Pointer validity
    /// section in the module documentation sets out every rule.
    ///
    /// A caller looks at a component by borrowing it through the pointer. The pointer method `read`
    /// makes a bitwise copy, and the vector still owns the original. The caller MUST guarantee that
    /// it never drops such a copy where the type has clean-up code. The vector later drops the
    /// original as well, which makes that a double drop, and a double drop is undefined behaviour.
    ///
    /// # Panics
    ///
    /// A debug build panics where `index` times the component size overflows `usize`. A release
    /// build wraps around instead, meaning the pointer can land inside the block.
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
            "a vector for a component type of size zero reached its limit of usize::MAX components"
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
            // SAFETY: `realloc` has four requirements, and the call meets each one. The assert
            // above means the type takes at least one byte.
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
    /// The caller MUST guarantee three things:
    ///
    /// - `value` points at a live component of the type that the vector stores.
    /// - `value` points outside the block of this vector, since growing can free the block.
    /// - Nothing drops the original afterwards, and nothing uses it unless its type is `Copy`.
    unsafe fn push(&mut self, value: *const u8) {
        if self.len == self.capacity {
            self.grow();
        }

        let end = self.ptr_at(self.len); // points just past the last component
        // SAFETY: `copy_nonoverlapping` requires two valid ranges that do not overlap. The caller
        // MUST guarantee that `value` is valid and lies outside the block, as the Safety section
        // above sets out. `end` has room for one
        // component, since the check above grows a full block. For a type of size zero, the copy
        // covers zero bytes and does not touch any memory
        unsafe { ptr::copy_nonoverlapping(value, end, self.info.layout.size()) }
        self.len += 1;
    }

    /// Moves `value` onto the end of the vector, which then owns it.
    ///
    /// The vector grows its block where the block is full.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores, or where the vector is full
    /// and a larger block would exceed `isize::MAX` bytes. A vector for a type of size zero panics
    /// once it stores `usize::MAX` components. The program aborts where the allocator cannot supply
    /// the larger block.
    #[track_caller]
    pub(crate) fn push_typed<T: 'static>(&mut self, value: T) {
        self.assert_stores::<T>();

        let value = ManuallyDrop::new(value);
        let bytes = ptr::from_ref(&*value).cast::<u8>();
        // SAFETY: `push` requires a live component of the stored type (outside the block) that
        // nothing drops afterwards. `assert_stores` confirms the type. `value` lies on the stack,
        // and `ManuallyDrop` stops it from being dropped here
        unsafe { self.push(bytes) }
    }

    /// Returns every component in the vector as a slice of `T`, in index order.
    ///
    /// For an empty vector, the method returns an empty slice. The slice borrows the vector, meaning the compiler
    /// rejects a push or a move while the slice is in use.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores.
    #[track_caller]
    pub(crate) fn as_slice<T: 'static>(&self) -> &[T] {
        self.assert_stores::<T>();

        // SAFETY: `from_raw_parts` requires an aligned pointer to `len` live values of `T`, which
        // nothing changes while the slice exists. `assert_stores` confirms the type, `data` is
        // always aligned, and every index below `len` stores a live component. The borrow of `&self`
        // stops safe code from changing them. Code that writes through a raw pointer MUST guarantee
        // that nothing else accesses them. An empty vector gives an empty slice, which does not read
        // any memory
        unsafe { slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Returns every component in the vector as a slice of `T` that allows changes, in index order.
    ///
    /// For an empty vector, the method returns an empty slice. The slice borrows the vector
    /// exclusively, meaning
    /// the compiler rejects any other use of the vector while the slice is in use.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores.
    #[track_caller]
    pub(crate) fn as_mut_slice<T: 'static>(&mut self) -> &mut [T] {
        self.assert_stores::<T>();

        // SAFETY: `from_raw_parts_mut` has the same requirements as `from_raw_parts` in `as_slice`,
        // which hold here for the same reasons. It also requires that nothing else accesses those
        // components while the slice exists, and the borrow of `&mut self` guarantees that
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Checks that `T` is the component type that the vector stores.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the stored type. The message contains both type names, and the
    /// panic reports the line that called the outermost `#[track_caller]` method.
    #[track_caller]
    fn assert_stores<T: 'static>(&self) {
        assert_eq!(
            self.info.type_id,
            TypeId::of::<T>(),
            "the vector stores the component type `{}`, but the caller requested `{}`",
            self.info.type_name,
            any::type_name::<T>()
        );
    }

    /// Moves the component at `index` onto the end of `target`, then fills `index` with the last
    /// component in `self`.
    ///
    /// `self` ends up one component shorter. `target` owns the moved component. The component that
    /// was last is now stored at `index`. The caller then updates the location that it records for
    /// the entity that owns that component. Where `index` is the last index, no other component
    /// moves, and the caller does not update any other entity.
    ///
    /// # Panics
    ///
    /// Panics where `target` is full and a larger block would exceed `isize::MAX` bytes. A target
    /// for a type of size zero panics once it stores `usize::MAX` components. The program aborts
    /// where the allocator cannot supply a larger block for `target`.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee two things:
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

        // SAFETY: `push` requires a live component of the type in `target`. The caller MUST
        // guarantee both, under the Safety section above. Both references are exclusive, which makes `target` a separate vector.
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

    /// Drops the component at `index`, then fills `index` with the component that was last.
    ///
    /// `self` ends up one component shorter. The caller then updates the location that it records
    /// for the entity that owns the component now at `index`. Where `index` is the last index, no
    /// other component moves, and the caller does not update any other entity.
    ///
    /// # Panics
    ///
    /// Panics where `index` is not below the length, before anything changes.
    ///
    /// Also panics where the `Drop` implementation of the component panics. The vector is already
    /// one component shorter at that point, and stays safe to use.
    pub(crate) fn swap_remove(&mut self, index: usize) {
        // Panics before anything changes, where `index` lies past the last component
        assert!(index < self.len, "the index must be below the length");

        // Finds the index of the last component
        let last_idx = self.len - 1;
        // Swaps the removed component with the last one, where the two differ
        if index != last_idx {
            // Measures one component in bytes, which is how many bytes the swap exchanges
            let size = self.info.layout.size();
            // SAFETY: `swap_nonoverlapping` requires two valid ranges that do not overlap. `index` and
            // `last_idx` are different indices below the length, meaning each range lies inside the block,
            // apart from the other. For a type of size zero, each range covers zero bytes
            unsafe { ptr::swap_nonoverlapping(self.ptr_at(index), self.ptr_at(last_idx), size) };
        }
        // Shortens the vector by one, which puts the removed component, now at `last_idx`, out of reach
        self.len = last_idx;

        // Runs the removed component's clean-up code, where its type has any
        if let Some(drop_component) = self.info.drop {
            // SAFETY: `drop_component` requires a live, aligned component that nothing uses afterwards.
            // Address `last_idx` stores the live component from `index`, and is aligned like every other
            // index. The length now excludes it, meaning the vector never reaches it again
            unsafe { drop_component(self.ptr_at(last_idx)) };
        }
    }
}

impl Drop for ComponentVec {
    /// Drops every component in the vector, then frees the block.
    ///
    /// Where the clean-up code of a component panics, the method stops at that component. The
    /// components after it never get dropped, and the block never gets freed. This is a memory leak
    /// rather than undefined behaviour. Sarx accepts the leak as a decision. A panicking clean-up is
    /// a bug in the component, and an uncaught panic on the main thread ends the program.
    fn drop(&mut self) {
        if let Some(drop_component) = self.info.drop {
            for index in 0..self.len {
                // SAFETY: `drop_component` requires a live, aligned component that nothing uses
                // afterwards. Every index below `len` stores a live component. Each address is
                // aligned for the type, since `data` is aligned and the component size is a
                // multiple of the alignment. `drop` runs once, and no code can use the vector after
                // it
                unsafe {
                    drop_component(self.ptr_at(index));
                }
            }
        }

        // Frees the block, which exists only for a type with a size above zero and a capacity above
        // zero
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
// `unsafe` write through a raw pointer, whose caller MUST guarantee that nothing else accesses that
// component
unsafe impl Send for ComponentVec {}
// SAFETY: the comment on `Send` above covers `Sync` as well
unsafe impl Sync for ComponentVec {}

/// Describes a component type to code that knows the type only at run time.
///
/// Most of the fields are facts about the type. The bit is the exception, since one world assigns
/// it. A `ComponentInfo` therefore belongs to the world that built it.
///
/// # Examples
///
/// The example builds the `ComponentInfo` for `f32`, with the mask value `1` for that type.
///
/// ```ignore
/// // Builds the `ComponentInfo` for `f32`, a type with no clean-up code
/// let info = ComponentInfo::of::<f32>(1);
/// ```
#[derive(Clone, Copy, Debug)]
pub(crate) struct ComponentInfo {
    /// Identifies the component type.
    type_id: TypeId,
    /// Stores the name of the component type, which a panic message reports.
    type_name: &'static str,
    /// Defines the size and alignment of one component.
    layout: Layout,
    /// Stores the function that runs the clean-up code of the component at the given address. The
    /// field is `None` where the type has no clean-up code.
    drop: Option<unsafe fn(*mut u8)>,
    /// Stores the bit that stands for the component type in a table's mask, within one world.
    bit: u64,
}

impl ComponentInfo {
    /// Returns the [`ComponentInfo`] for the component type `T`, with `bit` as its bit in a table's
    /// mask.
    ///
    /// The caller MUST pass a `u64` with exactly one set bit, and a different bit for each component
    /// type in one world. The method stores `bit` without checking it. A wrong bit gives a table a
    /// wrong mask, and queries then match the wrong tables.
    ///
    /// The bound `T: 'static` means that `T` contains no borrowed references that could expire. A
    /// vector can then keep a `T` for as long as it needs. `TypeId::of` requires the same bound.
    ///
    /// The bound `T: Send + Sync` lets a vector move its components between threads, and share them.
    pub(crate) fn of<T: Send + Sync + 'static>(bit: u64) -> Self {
        let drop = if mem::needs_drop::<T>() {
            Some(drop_value::<T> as unsafe fn(*mut u8))
        } else {
            None
        };

        Self {
            type_id: TypeId::of::<T>(),
            type_name: any::type_name::<T>(),
            layout: Layout::new::<T>(),
            drop,
            bit,
        }
    }

    /// Returns the [`TypeId`] that identifies the component type.
    pub(crate) fn type_id(self) -> TypeId {
        self.type_id
    }

    /// Returns the bit that stands for the component type in a table's mask.
    pub(crate) fn bit(self) -> u64 {
        self.bit
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Drops the `T` that `pointer` points at.
///
/// # Safety
///
/// The caller MUST guarantee two things:
///
/// - `pointer` points at a live `T`, at an address that is correctly aligned for `T`.
/// - Nothing uses or drops that `T` afterwards, since its clean-up code has already run.
unsafe fn drop_value<T>(pointer: *mut u8) {
    // SAFETY: `drop_in_place` requires a pointer to a live `T`, at an address that is correctly
    // aligned for `T`. The caller MUST guarantee both, under the Safety section above. The caller
    // MUST also guarantee that nothing uses the `T` afterwards, since its clean-up code runs here
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
        .expect("the block size should fit in a `usize`");
    Layout::from_size_align(size, item.align()).expect("the block should be at most isize::MAX bytes")
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

        // Pairs each `ComponentInfo` with the size, alignment and clean-up that it should report
        let cases = [
            ("f32", ComponentInfo::of::<f32>(1), 4, 4, false),
            (
                "String",
                ComponentInfo::of::<String>(1),
                size_of::<String>(),
                align_of::<String>(),
                true,
            ),
            ("()", ComponentInfo::of::<()>(1), 0, 1, false),
            ("DropOnly", ComponentInfo::of::<DropOnly>(1), 0, 1, true),
            ("Aligned", ComponentInfo::of::<Aligned>(1), 64, 64, false),
        ];

        for (name, info, size, alignment, has_drop) in cases {
            assert_eq!(info.layout.size(), size, "size of {name}");
            assert_eq!(info.layout.align(), alignment, "alignment of {name}");
            assert_eq!(info.drop.is_some(), has_drop, "drop function of {name}");
        }
    }

    /// Checks that two types with the same layout get different type IDs
    #[test]
    fn of_with_same_layout_types_returns_different_type_ids() {
        // Defines two separate types with identical fields
        #[allow(dead_code)]
        struct Health(u32);
        #[allow(dead_code)]
        struct Armour(u32);

        let health = ComponentInfo::of::<Health>(1);
        let armour = ComponentInfo::of::<Armour>(1);

        assert_eq!(health.layout, armour.layout, "layouts should match");
        assert_ne!(health.type_id, armour.type_id, "type IDs should differ");
    }

    /// Checks the starting state that a vector has before it allocates any memory
    #[test]
    fn new_with_standard_types_returns_empty_vector_with_aligned_pointer() {
        let cases = [
            ("u8", ComponentInfo::of::<u8>(1)),
            ("f32", ComponentInfo::of::<f32>(1)),
            ("u64", ComponentInfo::of::<u64>(1)),
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
        let components = ComponentVec::new(ComponentInfo::of::<f32>(1));
        let slice = components.as_slice::<f32>();

        assert!(slice.is_empty(), "slice should have no components");
    }

    /// Checks that a component survives being stored as bytes and read back as its own type
    #[test]
    fn push_typed_with_one_value_stores_value_for_as_slice() {
        let mut components = ComponentVec::new(ComponentInfo::of::<f32>(1));

        components.push_typed(1.5_f32);

        assert_eq!(components.as_slice::<f32>(), [1.5]);
    }

    /// Checks that `push` stores a `String` that the vector then owns, and reads it back unchanged
    #[test]
    fn push_with_manually_dropped_string_stores_value_for_as_slice() {
        let mut components = ComponentVec::new(ComponentInfo::of::<String>(1));
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
        let components = ComponentVec::new(ComponentInfo::of::<u32>(1));
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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));
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
            let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));

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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));

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
        let mut components = ComponentVec::new(ComponentInfo::of::<String>(1));

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
            let mut components = ComponentVec::new(ComponentInfo::of::<DropCounter>(1));
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
        let mut components = ComponentVec::new(ComponentInfo::of::<DropCounter>(1));

        // Pushes 9 components, which makes the block grow from 4 to 8, then from 8 to 16
        for _ in 0..9 {
            components.push_typed(DropCounter {
                drops: Arc::clone(&drops),
            });
        }

        assert_eq!(
            drops.load(Ordering::Relaxed),
            0,
            "drops while the vector is alive"
        );
    }

    /// Checks that the middle component moves to the target, and the last component fills its index
    #[test]
    fn move_component_with_middle_index_fills_gap_with_last_component() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(1));
        source.push_typed(10_u32);
        source.push_typed(20_u32);
        source.push_typed(30_u32);
        source.push_typed(40_u32);

        let mut target = ComponentVec::new(source.info());

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 1 is below the length of 4, and `target` uses the same `ComponentInfo`
        unsafe { source.move_component(1, &mut target) };

        assert_eq!(
            source.as_slice::<u32>(),
            [10, 40, 30],
            "source after the move"
        );
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
            let mut source = ComponentVec::new(ComponentInfo::of::<u32>(1));
            for &value in values {
                source.push_typed(value);
            }

            let mut target = ComponentVec::new(source.info());

            // SAFETY: `move_component` requires an index below the length and a target of the same
            // type. Each case gives its last index, and `target` uses the same `ComponentInfo`
            unsafe { source.move_component(index, &mut target) };

            assert_eq!(source.as_slice::<u32>(), remaining, "source for {values:?}");
            assert_eq!(
                target.as_slice::<u32>(),
                [values[index]],
                "target for {values:?}"
            );
        }
    }

    /// Checks that a move into a full target grows the target and keeps what it already stored
    #[test]
    fn move_component_with_full_target_grows_target() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(1));
        source.push_typed(50_u32);

        let mut target = ComponentVec::new(source.info());
        for value in [10_u32, 20, 30, 40] {
            target.push_typed(value);
        }

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 0 is below the length of 1, and `target` uses the same `ComponentInfo`
        unsafe { source.move_component(0, &mut target) };

        assert_eq!(target.capacity, 8, "target capacity after the move");
        assert_eq!(
            target.as_slice::<u32>(),
            [10, 20, 30, 40, 50],
            "target after the move"
        );
    }

    /// Checks that a move passes ownership on, meaning two vectors never clean up the same component
    #[test]
    fn move_component_with_drop_counters_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut source = ComponentVec::new(ComponentInfo::of::<DropCounter>(1));
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
        let mut components = ComponentVec::new(ComponentInfo::of::<()>(1));
        let start = components.data();

        for _ in 0..5 {
            components.push_typed(());
        }

        assert_eq!(components.as_slice::<()>().len(), 5, "number of components");
        assert_eq!(components.capacity, usize::MAX, "capacity");
        assert_eq!(components.data(), start, "pointer to the block");
    }

    /// Checks that a type taking no memory still gets a pointer that meets its alignment
    #[test]
    fn as_slice_with_aligned_zero_size_type_returns_aligned_slice() {
        // Defines a type of size zero with an alignment that most addresses fail
        #[repr(align(64))]
        struct Tag;

        let mut components = ComponentVec::new(ComponentInfo::of::<Tag>(1));
        for _ in 0..3 {
            components.push_typed(Tag);
        }

        let slice = components.as_slice::<Tag>();
        let remainder = slice.as_ptr().addr() % 64;

        assert_eq!(slice.len(), 3, "number of components");
        assert_eq!(remainder, 0, "remainder of the address divided by 64");
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

        let mut components = ComponentVec::new(ComponentInfo::of::<Marker>(1));
        for _ in 0..3 {
            components.push_typed(Marker);
        }

        drop(components);

        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    /// Checks that a type taking no memory moves by count alone, and is cleaned up once per
    /// component
    #[test]
    fn move_component_with_zero_size_drop_type_drops_each_component_once() {
        // Counts the drops outside the type, since a type of size zero has no room for a counter
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Marker;
        impl Drop for Marker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut source = ComponentVec::new(ComponentInfo::of::<Marker>(1));
        for _ in 0..3 {
            source.push_typed(Marker);
        }

        let mut target = ComponentVec::new(source.info());

        // SAFETY: `move_component` requires an index below the length and a target of the same
        // type. Index 0 is below the length of 3, and `target` uses the same `ComponentInfo`
        unsafe { source.move_component(0, &mut target) };

        let source_len = source.as_slice::<Marker>().len();
        let target_len = target.as_slice::<Marker>().len();
        let drops_after_move = DROPS.load(Ordering::Relaxed);

        drop(source);
        drop(target);
        let drops_after_vectors = DROPS.load(Ordering::Relaxed);

        assert_eq!(source_len, 2, "components left in the source");
        assert_eq!(target_len, 1, "components in the target");
        assert_eq!(drops_after_move, 0, "drops caused by the move");
        assert_eq!(drops_after_vectors, 3, "drops after both vectors are gone");
    }

    /// Checks that a vector of a type that takes no memory stops at its limit, rather than
    /// allocating
    #[test]
    #[should_panic]
    fn push_typed_with_full_zero_size_vector_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<()>(1));

        // Sets the length by hand, since pushing `usize::MAX` components would take too long
        components.len = usize::MAX;

        components.push_typed(());
    }

    /// Checks that a type with a large alignment stays aligned at every index as the block grows
    #[test]
    fn push_typed_with_large_alignment_keeps_every_component_aligned() {
        #[repr(align(64))]
        struct Aligned(u8);

        let mut components = ComponentVec::new(ComponentInfo::of::<Aligned>(1));

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

        assert_eq!(
            remainders, [0; 9],
            "remainder of each address divided by 64"
        );
        assert_eq!(values, [0, 1, 2, 3, 4, 5, 6, 7, 8], "stored values");
    }

    /// Checks that pushing a value of another type panics, even at the same size
    #[test]
    #[should_panic]
    fn push_typed_with_wrong_type_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));

        components.push_typed(1.5_f32);
    }

    /// Checks that reading the stored bytes as another type panics, even at the same size
    #[test]
    #[should_panic]
    fn as_slice_with_wrong_type_panics() {
        let components = ComponentVec::new(ComponentInfo::of::<u32>(1));

        components.as_slice::<f32>();
    }

    /// Checks that changing the stored bytes as another type panics, even at the same size
    #[test]
    #[should_panic]
    fn as_mut_slice_with_wrong_type_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));

        components.as_mut_slice::<f32>();
    }

    /// Checks that a debug build catches an index that would read past the last component
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn move_component_with_out_of_range_index_panics_in_debug_build() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(1));
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(source.info());

        // SAFETY: this call breaks the index requirement on purpose. The `debug_assert` in
        // `move_component` panics before any memory is read
        unsafe { source.move_component(1, &mut target) };
    }

    /// Checks that a debug build catches a move between columns of different types
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic]
    fn move_component_with_mismatched_target_panics_in_debug_build() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(1));
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(ComponentInfo::of::<f32>(1));

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
    #[should_panic]
    fn array_layout_with_overflowing_size_panics() {
        array_layout(Layout::new::<u64>(), usize::MAX);
    }

    /// Checks the limit when the block size fits in `usize`, but passes the largest size allowed
    #[test]
    #[should_panic]
    fn array_layout_with_size_above_limit_panics() {
        let count = isize::MAX as usize + 1;

        array_layout(Layout::new::<u8>(), count);
    }

    /// Checks that threads can read one vector together, and that a vector can move to a thread
    #[test]
    fn as_slice_with_several_threads_returns_same_components() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(1));
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
        let mut source_health = ComponentVec::new(ComponentInfo::of::<u32>(1));
        let mut source_names = ComponentVec::new(ComponentInfo::of::<String>(1));
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
        assert_eq!(
            source_names.as_slice::<String>(),
            ["Cy", "Bo"],
            "source names"
        );
        assert_eq!(target_health.as_slice::<u32>(), [10], "target health");
        assert_eq!(target_names.as_slice::<String>(), ["Ada"], "target names");
    }
}
