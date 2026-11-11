//! Defines [`Component`] and [`ComponentVec`], the trait that marks a type as a component, and the vector that stores
//! the components of one type, respectively.
//!
//! An archetype is one set of component types, modelled as a table with one row for each entity
//! and one column for each component type. [`ComponentVec`] represents a column in this
//! **archetype table**.
//!
//! A [`ComponentVec`] stores its components next to each other, in one block of memory. A table
//! built from these vectors therefore has column-major order, meaning that
//! neighbouring components in a column are also neighbours in memory. Contrastingly, the components of one row lie
//! in separate blocks, one block per column.
//!
//! A table learns its component types at run time, as ADR 0011 sets out. A generic `Vec<T>` needs
//! its type at compile time, meaning it cannot serve as a column. Therefore, a [`ComponentVec`]
//! stores its components as raw bytes.
//!
//! A [`ComponentInfo`] describes those bytes to [`ComponentVec`]. It stores the memory layout, type name, type identity,
//! and drop function of its [`Component`]. It also stores the bit that the world gives its
//! component type. Each table combines the bits of its types into a mask, which the
//! [`archetype`](crate::archetype) module describes in more detail.
//!
//! The compiler cannot check the type of a [`ComponentVec`]'s bytes. As a result,
//! [`ComponentVec::push`], the one `unsafe` method, takes raw bytes and has a Safety section that
//! lists what its caller MUST guarantee. One of those guarantees is that the bytes belong to the
//! component type that the vector stores.
//!
//! # Typed and raw access
//!
//! A caller reaches the components in one of two ways. The choice depends on whether the caller
//! knows the component type at compile time.
//!
//! A caller that knows the type uses [`ComponentVec::as_slice`] or [`ComponentVec::as_mut_slice`].
//! Both methods panic when given a type that differs from the one the vector stores. The slice
//! also borrows the vector, meaning the compiler rejects a push while the slice is in use.
//! This is the safe way to reach a component.
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
//! The vector grows where [`ComponentVec::push_typed`] finds the block full, and where
//! [`ComponentVec::reserve`] needs more room than the block has. [`ComponentVec::move_component`]
//! pushes onto its target, meaning it can grow the target too.
//!
//! Dropping the vector frees its block. Every pointer into that block then stops being valid, and
//! using one is undefined behaviour.
//!
//! [`ComponentVec::move_component`] changes what some addresses contain. For example, a vector that
//! stores four components, `A`, `B`, `C` and `D`, at indices 0 to 3, and moves `B` out from index 1
//! to some other [`ComponentVec`] target, then copies `D` into index 1, and the length drops to 3.
//!
//! Two pointers taken before the move would now mislead their caller. A pointer to index 1 used to
//! reach `B`, and now reaches `D`. A pointer to index 3 still reaches a copy of `D`, but that copy
//! lies past the end of the vector, and the vector no longer counts it as a component. Dropping `D`
//! through that pointer drops it twice: once there, and again when the vector drops the `D` at
//! index 1. A double drop is undefined behaviour.
//!
//! [`ComponentVec::swap_remove`] leaves the same two pointers behind. It drops `B` instead of moving
//! it, then copies `D` into index 1 in the same way.
//!
//! A raw pointer has no lifetime, meaning the compiler accepts code that keeps one across
//! a push, a reserve, a move, a removal or the drop of the vector. A push or a reserve can grow the
//! block. The caller therefore MUST guarantee that it does not use any pointer from before a push,
//! a reserve, a move, a removal or the drop. To reach a component again, the caller requests a new
//! pointer by index. Components keep their indices when the vector grows. After a move or a removal
//! from an index other than the last, the component that was last is stored at that index.
//!
//! # Examples
//!
//! Each example below comes from a method in `enki` that uses a [`ComponentVec`], cut down to the
//! lines that touch the vector.
//!
//! ## Creating the columns of a table
//!
//! `Archetype::new` receives one [`ComponentInfo`] for each component type in the table, and
//! creates an empty column from each one. It then sorts the columns by type, reading each type
//! back through [`ComponentVec::info`].
//!
//! ```ignore
//! // Creates one empty column for each entry in `infos`
//! let mut components: Vec<ComponentVec> =
//!     infos.iter().map(|info| ComponentVec::new(*info)).collect();
//! // Sorts the columns by `TypeId`, which the binary search for a column requires
//! components.sort_by_key(|components| components.info().type_id());
//! ```
//!
//! ## Reading and changing one component
//!
//! `World::get` and `World::get_mut` know the component type `T` at compile time, and therefore
//! use the typed methods. Each one borrows the column as a slice of `T`, then indexes the slice
//! with the row of the entity.
//!
//! ```ignore
//! // Returns the entity's `T`, or `None` where the row lies past the end of the column
//! components.as_slice::<T>().get(location.row())
//!
//! // Returns the same component, for the caller to change
//! components.as_mut_slice::<T>().get_mut(location.row())
//! ```
//!
//! ## Adding or replacing a component
//!
//! `push_or_replace`, in the `set` module, gives an entity a component of type `T`. Where the
//! entity already has a `T`, the function swaps the new component into its row and returns the
//! old one. Otherwise it pushes the new component onto the end of the column.
//!
//! ```ignore
//! let bit = column.info().bit();
//! if existing.contains_any(bit) {
//!     // Puts `value` in place of the old component, and returns the old one to the caller
//!     let slot = &mut column.as_mut_slice::<T>()[row];
//!     return Some(mem::replace(slot, value));
//! }
//! // Moves `value` onto the end of the column, which then owns it
//! column.push_typed(value);
//! None
//! ```
//!
//! ## Moving an entity to another table
//!
//! `Archetype::move_entity` visits each column in the entity's old table. Where the target table
//! has a column of the same type, [`ComponentVec::move_component`] moves the component across.
//! Otherwise the column goes to the closure `take`, which removes the component.
//!
//! ```ignore
//! for components in &mut self.components {
//!     let type_id = components.info().type_id();
//!     match target.components_of_mut(type_id) {
//!         // Moves the component to the end of the target's column for the same type
//!         Some(target_components) => components.move_component(row, target_components),
//!         // Lets `take` remove the component, since the target has no column for its type
//!         None => take(components, row),
//!     }
//! }
//! ```
//!
//! The caller of the move decides what `take` does with the component. `World::remove_component`
//! keeps it as a value, through [`ComponentVec::swap_remove_typed`], and returns it.
//! `World::add_components` passes [`ComponentVec::swap_remove`] itself, which drops it.
//!
//! ```ignore
//! // Takes the entity's `T` out of the column, and keeps it to return to the caller
//! |components, row| removed = Some(components.swap_remove_typed::<T>(row))
//! ```
//!
//! ## Removing an entity
//!
//! `Archetype::remove_entity` calls [`ComponentVec::swap_remove`] on every column. Each call drops
//! the entity's component, and the last component in the column fills `row`. The entity list then
//! does the same, meaning the last entity and all of its components end up in `row` together.
//!
//! ```ignore
//! for components in &mut self.components {
//!     // Drops the entity's component, and fills `row` with the last component in the column
//!     components.swap_remove(row);
//! }
//! // Removes the entity from the list in the same way, filling `row` with the last entity
//! self.entities.swap_remove(row);
//! ```
//!
//! ## Reaching a column through its raw pointer
//!
//! A `&mut T` query receives the table through a shared reference, yet hands out a `&mut T` for
//! each component. It cannot call [`ComponentVec::as_mut_slice`], which needs a
//! `&mut ComponentVec`. Instead, its `find_columns` builds the slice from
//! [`ComponentVec::data`] and [`ComponentVec::len`].
//!
//! ```ignore
//! let components = archetype.components_of(TypeId::of::<T>())?;
//! let start = components.data().cast::<T>();
//! let len = components.len();
//! // SAFETY: `components_of` found the column for `T`, and every index below `len` stores a live
//! // component. The caller of `find_columns` MUST guarantee that nothing else reaches these
//! // components while the slice is in use
//! Some(unsafe { slice::from_raw_parts_mut(start.as_ptr(), len) })
//! ```

use crate::mask::ComponentMask;
use std::alloc::{self, Layout};
use std::any::{self, TypeId};
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::{mem, ptr, slice};

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Marks a type as a component, which a [`World`](crate::World) can store on an entity.
///
/// The game makes a type a component by implementing this trait for it, with one empty `impl`.
/// Every other type stays outside the world. As a result, a tuple passed to
/// `World::add_component` by mistake fails to compile, as ADR 0015 sets out.
///
/// The trait requires three other traits of every component type:
///
/// - `'static` means that any reference inside the type stays valid for the whole program. The
///   world can then keep a component for as long as it needs.
/// - `Send` lets the world move its components to another thread.
/// - `Sync` lets several threads read the same component at once.
///
/// # Examples
///
/// The example marks `Health` as a component, then gives an entity one.
///
/// ```
/// use enki::{Component, World};
///
/// #[derive(Debug)]
/// struct Health(u32);
///
/// // Marks `Health` as a component
/// impl Component for Health {}
///
/// let mut world = World::new();
/// let player = world.spawn_empty();
/// world.add_component(player, Health(100)).expect("the player should be alive");
///
/// let health = world.get::<Health>(player).map(|health| health.0);
/// assert_eq!(health, Some(100));
/// ```
///
/// The compiler rejects a type that does not implement the trait. The example passes a tuple to
/// `add_component` by mistake, in place of `add_components`, and fails to compile.
///
/// ```compile_fail
/// use enki::World;
/// # use enki::Component;
///
/// struct Position(f32, f32);
/// struct Health(u32);
/// # impl Component for Position {}
/// # impl Component for Health {}
///
/// let mut world = World::new();
/// let player = world.spawn_empty();
///
/// // Fails to compile, since the tuple `(Position, Health)` does not implement `Component`
/// world.add_component(player, (Position(0.0, 0.0), Health(100)));
/// ```
pub trait Component: Send + Sync + 'static {}

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
/// The [module examples](self#examples) show the vector as tables and the world use it: creating
/// columns, reading, pushing, moving and removing components. The test
/// `as_mut_slice_with_changed_value_updates_only_that_component` changes one component through
/// `as_mut_slice`.
pub(crate) struct ComponentVec {
    info: ComponentInfo, // describes the component type that the vector stores
    data: NonNull<u8>,   // points at the start of the block
    len: usize,          // counts the components in the vector
    capacity: usize,     // counts the components that the block has room for
}

impl ComponentVec {
    // ========================================================================================== \\
    //                                        Constructors                                        \\
    // ========================================================================================== \\

    /// Returns an empty vector for the component type that `info` describes.
    ///
    /// The vector allocates its block on the first push, move into this vector, or call to
    /// [`reserve`](Self::reserve) that needs room. A vector for a type of size zero starts with a
    /// capacity of `usize::MAX`, and never allocates.
    ///
    /// The [module example](self#creating-the-columns-of-a-table) creates one column for each
    /// component type in a table with this method.
    pub(crate) fn new(info: ComponentInfo) -> Self {
        // Points `data` at an aligned address, which stands in until the vector allocates a block
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

    // ========================================================================================== \\
    //                                          Getters                                           \\
    // ========================================================================================== \\

    /// Returns the [`ComponentInfo`] that describes the component type in the vector.
    ///
    /// The [module example](self#creating-the-columns-of-a-table) reads the type of each column
    /// through this method, to sort the columns of a table.
    pub(crate) fn info(&self) -> ComponentInfo {
        self.info
    }

    /// Returns a pointer to the start of the block, where index 0 begins.
    ///
    /// This method returns the same address as [`ptr_at`](Self::ptr_at) with an index of 0, as a
    /// `NonNull`, a pointer type that is never null.
    ///
    /// The pointer is always aligned for the component type. Until the vector allocates a block, the
    /// pointer does not point at any memory. The pointer stops being valid when the vector grows its
    /// block or is dropped. The Pointer validity section in the module documentation sets out every
    /// rule.
    ///
    /// # Examples
    ///
    /// For a vector with at least one component, `data` returns the same address as `ptr_at(0)`.
    /// The test `data_with_pushed_components_returns_address_of_first_component` checks it.
    pub(crate) fn data(&self) -> NonNull<u8> {
        self.data
    }

    /// Returns the number of components in the vector.
    ///
    /// # Examples
    ///
    /// After three pushes, `len` returns 3. The test
    /// `len_with_varying_counts_returns_number_of_components` checks several counts.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    // ========================================================================================== \\
    //                                        API Methods                                         \\
    // ========================================================================================== \\

    /// Returns a pointer to the start of the component at `index`.
    ///
    /// The method does not check bounds, meaning the pointer can lie past the end of the block.
    /// To read through the pointer, the caller MUST guarantee that `index` is below the length. The
    /// pointer stops being valid when the vector grows its block or is dropped. The Pointer validity
    /// section in the module documentation sets out every rule.
    ///
    /// A caller that only looks at a component borrows it through the pointer. Calling `read` on
    /// the pointer instead makes a bitwise copy, while the vector still owns the original. The
    /// caller MUST guarantee that it never drops such a copy where the type has clean-up code,
    /// since the vector later drops the original as well, and a double drop is undefined behaviour.
    ///
    /// # Panics
    ///
    /// A debug build panics where `index` times the component size overflows `usize`. A release
    /// build wraps around instead, meaning the pointer can land inside the block.
    ///
    /// # Examples
    ///
    /// For a vector of `f32`, `ptr_at(1)` returns the address 4 bytes past `data`, since each `f32`
    /// takes 4 bytes. The test `ptr_at_with_standard_and_extreme_indices_returns_offset_address`
    /// checks other indices.
    pub(crate) fn ptr_at(&self, index: usize) -> *mut u8 {
        let offset = index * self.info.layout.size();
        self.data.as_ptr().wrapping_add(offset)
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
    ///
    /// After such a panic, nothing drops `value`, and it leaks.
    ///
    /// The [module example](self#adding-or-replacing-a-component) pushes a new component onto the
    /// end of a column with this method.
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
    /// For an empty vector, the method returns an empty slice. The slice borrows the vector, meaning
    /// the compiler rejects a push or a move while the slice is in use.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores.
    ///
    /// The [module example](self#reading-and-changing-one-component) borrows a column with this
    /// method, to read the component of one entity.
    #[track_caller]
    pub(crate) fn as_slice<T: 'static>(&self) -> &[T] {
        self.assert_stores::<T>();

        // SAFETY: `from_raw_parts` requires an aligned pointer to `len` live values of `T`, which
        // nothing changes while the slice exists. `assert_stores` confirms the type, `data` is
        // always aligned, and every index below `len` stores a live component. The borrow of `&self`
        // stops safe code from changing them. Code that writes through a raw pointer MUST guarantee
        // that nothing else accesses them. The slice also MUST span at most `isize::MAX` bytes, and
        // `array_layout` keeps every block within that size. An empty vector gives an empty slice,
        // which does not read any memory
        unsafe { slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Returns every component in the vector as a slice of `T` that allows changes, in index order.
    ///
    /// For an empty vector, the method returns an empty slice. The slice borrows the vector
    /// exclusively, meaning the compiler rejects any other use of the vector while the slice is in
    /// use.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores.
    ///
    /// # Examples
    ///
    /// The test `as_mut_slice_with_changed_value_updates_only_that_component` changes one component
    /// through the slice, and checks that the others keep their values.
    #[track_caller]
    pub(crate) fn as_mut_slice<T: 'static>(&mut self) -> &mut [T] {
        self.assert_stores::<T>();

        // SAFETY: `from_raw_parts_mut` has the same requirements as `from_raw_parts` in `as_slice`,
        // which are met here for the same reasons. It also requires that nothing else accesses those
        // components while the slice exists, and the borrow of `&mut self` guarantees that
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr().cast::<T>(), self.len) }
    }

    /// Moves the component at `index` onto the end of `target`, then fills `index` with the last
    /// component in `self`.
    ///
    /// `self` ends up one component shorter, `target` owns the moved component, and the component
    /// that was last is now stored at `index`. The caller MUST then update the location that it
    /// records for the entity that owns that component. Where `index` is the last index, no other
    /// component moves, and the caller does not update any other entity.
    ///
    /// # Panics
    ///
    /// Panics where `index` is not below the length of `self`, or where `target` stores a different
    /// component type. Both checks run in every build, before anything changes.
    ///
    /// Also panics where `target` is full and a larger block would exceed `isize::MAX` bytes. A
    /// target for a type of size zero panics once it stores `usize::MAX` components. The program
    /// aborts where the allocator cannot supply a larger block for `target`.
    ///
    /// The [module example](self#moving-an-entity-to-another-table) moves each component of an
    /// entity to its new table with this method.
    pub(crate) fn move_component(&mut self, index: usize, target: &mut ComponentVec) {
        // Panics before anything changes, where `index` lies past the last component. A stale row
        // from a caller would otherwise copy the bytes of a component that is already dropped
        assert!(index < self.len, "the index must be below the length");
        // Panics before anything changes, where the two vectors store different component types
        assert_eq!(
            self.info.type_id, target.info.type_id,
            "both vectors must store the same component type"
        );

        // SAFETY: `push` requires a live component of the type that `target` stores, outside the
        // block of `target`, that nothing uses or drops afterwards. The first assert above confirms
        // that `index` is below the length, which makes the component live. The second confirms
        // that the two types match. `self` and `target` are both exclusive references, and two
        // exclusive references never reach the same vector. As a result, the pointer lies outside
        // the block of `target`. The fill below overwrites the original, or the shorter length puts
        // it out of reach. Nothing therefore uses or drops it afterwards
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
    /// `self` ends up one component shorter. The caller MUST then update the location that it
    /// records for the entity that owns the component now at `index`. Where `index` is the last
    /// index, no other component moves, and the caller does not update any other entity.
    ///
    /// # Panics
    ///
    /// Panics where `index` is not below the length, before anything changes.
    ///
    /// Also panics where the `Drop` implementation of the component panics. The vector is already
    /// one component shorter at that point, and stays safe to use.
    ///
    /// # Examples
    ///
    /// For a vector of `"Ada"`, `"Bo"` and `"Cy"`, `swap_remove(0)` drops `"Ada"` and moves `"Cy"`
    /// into index 0, which leaves `["Cy", "Bo"]`. The test
    /// `swap_remove_with_non_last_index_fills_gap_with_last_component` checks it.
    pub(crate) fn swap_remove(&mut self, index: usize) {
        // Panics before anything changes, where `index` lies past the last component
        assert!(index < self.len, "the index must be below the length");

        // Finds the index of the last component
        let last_idx = self.len - 1;
        // Swaps the removed component with the last one, where the two differ
        if index != last_idx {
            // Measures one component in bytes, which is how many bytes the swap exchanges
            let size = self.info.layout.size();
            // SAFETY: `swap_nonoverlapping` requires two valid ranges that do not overlap. `index`
            // and `last_idx` are different indices below the length, meaning each range lies
            // inside the block, apart from the other. For a type of size zero, each range covers
            // zero bytes
            unsafe { ptr::swap_nonoverlapping(self.ptr_at(index), self.ptr_at(last_idx), size) };
        }
        // Shortens the vector by one, which puts the removed component, now at `last_idx`, out of
        // reach
        self.len = last_idx;

        // Runs the removed component's clean-up code, where its type has any
        if let Some(drop_component) = self.info.drop {
            // SAFETY: `drop_component` requires a live, aligned component that nothing uses
            // afterwards. The address at `last_idx` stores the live component from `index`, and is
            // aligned like the address at every other index. The length now excludes `last_idx`,
            // meaning the vector never reaches that component again
            unsafe { drop_component(self.ptr_at(last_idx)) };
        }
    }

    /// Moves the component at `index` out of the vector and returns it, then fills `index` with the
    /// component that was last.
    ///
    /// The method works as [`swap_remove`](Self::swap_remove) does, apart from the end. Where
    /// `swap_remove` drops the component, this method hands it to the caller, which then owns it.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the type that the vector stores, or where `index` is not below
    /// the length. Both checks run before anything changes.
    ///
    /// # Examples
    ///
    /// For a vector of `"Ada"`, `"Bo"` and `"Cy"`, `swap_remove_typed::<String>(0)` returns `"Ada"`
    /// and moves `"Cy"` into index 0, which leaves `["Cy", "Bo"]`.
    #[track_caller]
    pub(crate) fn swap_remove_typed<T: 'static>(&mut self, index: usize) -> T {
        self.assert_stores::<T>();
        // Panics before anything changes, where `index` lies past the last component
        assert!(index < self.len, "the index must be below the length");

        let last_idx = self.len - 1;
        // Swaps the removed component with the last one, where the two differ
        if index != last_idx {
            let size = self.info.layout.size();
            // SAFETY: `swap_nonoverlapping` requires two valid ranges that do not overlap. `index`
            // and `last_idx` are different indices below the length, meaning each range lies
            // inside the block, apart from the other. For a type of size zero, each range covers
            // zero bytes
            unsafe { ptr::swap_nonoverlapping(self.ptr_at(index), self.ptr_at(last_idx), size) };
        }
        // Shortens the vector by one, which puts the removed component, now at `last_idx`, out of
        // reach
        self.len = last_idx;

        // SAFETY: `read` requires an aligned pointer to a live `T`, and makes a bitwise copy.
        // `assert_stores` confirms the type. The address at `last_idx` stores the live component
        // from `index`, and is aligned like the address at every other index. The length now
        // excludes `last_idx`, meaning the vector never reaches or drops the original, and the
        // copy is the only owner
        unsafe { self.ptr_at(last_idx).cast::<T>().read() }
    }

    /// Makes room for at least `additional` more components.
    ///
    /// Where the block lacks the room, the vector moves to a larger block. The new capacity is the
    /// larger of the capacity that the caller needs and double the current capacity. A vector for
    /// a type of size zero always has the room, and keeps its block.
    ///
    /// The components keep their indices. Where the vector moves to a larger block, every pointer
    /// that [`data`](Self::data) and [`ptr_at`](Self::ptr_at) returned earlier stops being valid.
    /// Using one of those pointers afterwards is undefined behaviour.
    ///
    /// # Panics
    ///
    /// Panics where the length plus `additional` overflows a `usize`, or where the new block would
    /// exceed `isize::MAX` bytes. The program aborts where the allocator cannot supply the block.
    pub(crate) fn reserve(&mut self, additional: usize) {
        // Works out the capacity that the caller needs, and panics where it overflows `usize`
        let required = self
            .len
            .checked_add(additional)
            .expect("the required capacity should fit in a `usize`");
        // Returns where the block already has the room, which is always true for a type of size zero
        if required <= self.capacity {
            return;
        }

        // Grows to at least double, which keeps repeated small reserves from copying every time
        let doubled = self.capacity.saturating_mul(2);
        self.increase_capacity(required.max(doubled));
    }

    // ========================================================================================== \\
    //                                       Helper Methods                                       \\
    // ========================================================================================== \\

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
    /// After such a panic, the vector does not own the component. The third guarantee below still
    /// binds the caller, meaning nothing drops the original, and the component leaks.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee three things:
    ///
    /// - `value` points at a live component of the type that the vector stores.
    /// - `value` points outside the block of this vector, since growing can free the block.
    /// - Nothing drops the original afterwards, and nothing uses it unless its type is `Copy`.
    ///
    /// # Examples
    ///
    /// `push_typed` shows a call, along with the SAFETY comment that the call needs. The test
    /// `push_with_manually_dropped_string_stores_value_for_as_slice` pushes a `String` that
    /// `ManuallyDrop` keeps from being dropped twice.
    unsafe fn push(&mut self, value: *const u8) {
        if self.len == self.capacity {
            self.grow();
        }

        let end = self.ptr_at(self.len); // points just past the last component
        // SAFETY: `copy_nonoverlapping` requires two valid ranges that do not overlap. The caller
        // MUST guarantee that `value` is valid and lies outside the block, as the Safety section
        // above sets out. `end` has room for one component, since the check above grows a full
        // block. For a type of size zero, the copy covers zero bytes and does not touch any memory
        unsafe { ptr::copy_nonoverlapping(value, end, self.info.layout.size()) }
        self.len += 1;
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
    ///
    /// # Examples
    ///
    /// An empty vector of `u32` grows to a capacity of 4 on the first call, and to 8 on the second.
    /// The test `push_typed_with_increasing_counts_returns_expected_capacity` checks the capacity
    /// as the vector fills.
    fn grow(&mut self) {
        // Keeps a type of size zero away from the allocator, which requires a size above zero
        self.assert_non_zero();

        let new_capacity = if self.capacity == 0 {
            4
        } else {
            self.capacity * 2
        };

        self.increase_capacity(new_capacity);
    }

    /// Moves the components to a block with room for `new_capacity` components. Allocates the first
    /// block where the capacity is 0.
    ///
    /// The components keep their indices. Every pointer that [`data`](Self::data) and
    /// [`ptr_at`](Self::ptr_at) returned earlier stops being valid, even where the block stays at
    /// the same address. Using one of those pointers afterwards is undefined behaviour.
    ///
    /// The caller must pass a `new_capacity` above the current capacity. A debug build panics where
    /// it does not.
    ///
    /// # Panics
    ///
    /// Panics where the component type has a size of zero, or where the new block would exceed
    /// `isize::MAX` bytes. The program aborts where the allocator cannot supply the block.
    ///
    /// # Examples
    ///
    /// For an empty vector of `u32`, `increase_capacity(10)` allocates a block of 40 bytes, sets
    /// the capacity to 10, and leaves the length at 0. The test
    /// `reserve_with_standard_and_extreme_amounts_returns_expected_capacity` reaches it through
    /// `reserve`.
    fn increase_capacity(&mut self, new_capacity: usize) {
        // Panics in a debug build where the caller does not ask for more room than the block has
        debug_assert!(
            new_capacity > self.capacity,
            "the new capacity must be above the current capacity"
        );
        // Keeps a type of size zero away from the allocator, which requires a size above zero
        self.assert_non_zero();

        let new_layout = array_layout(self.info.layout, new_capacity);

        let new_data = if self.capacity == 0 {
            // SAFETY: `alloc` requires a layout with a size above zero. The assert above means the
            // type takes at least one byte, and `new_capacity` is above a capacity of 0. As a
            // result, `new_layout` takes at least one byte
            unsafe { alloc::alloc(new_layout) }
        } else {
            let old_layout = array_layout(self.info.layout, self.capacity);
            // SAFETY: `realloc` has four requirements, and the call meets each one. The assert
            // above means the type takes at least one byte.
            //
            // - The global allocator allocated the block. The capacity is above zero, meaning an
            //   earlier call to `increase_capacity` allocated it.
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

    /// Checks that `T` is the component type that the vector stores.
    ///
    /// # Panics
    ///
    /// Panics where `T` differs from the stored type. The message contains both type names, and the
    /// panic reports the line that called the outermost `#[track_caller]` method.
    ///
    /// # Examples
    ///
    /// For a vector of `f32`, `assert_stores::<f32>()` returns, and `assert_stores::<u32>()`
    /// panics. The test `push_typed_with_wrong_type_panics` reaches the panic through `push_typed`.
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

    /// Checks that the component type takes at least one byte.
    ///
    /// A vector for a type of size zero starts with a capacity of `usize::MAX`. The only way it
    /// reaches this check is through [`grow`](Self::grow), once it stores that many components.
    ///
    /// # Panics
    ///
    /// Panics where the component type has a size of zero.
    ///
    /// # Examples
    ///
    /// For a vector of `f32`, which takes 4 bytes, the method returns. For a vector of `()`, a type
    /// of size zero, it panics. The test `push_typed_with_full_zero_size_vector_panics` reaches the
    /// panic through `push_typed`.
    fn assert_non_zero(&self) {
        assert_ne!(
            self.info.layout.size(),
            0,
            "a vector for a component type of size zero reached its limit of usize::MAX components"
        );
    }
}

impl Drop for ComponentVec {
    /// Drops every component in the vector, then frees the block.
    ///
    /// Where the clean-up code of a component panics, the method stops at that component. The
    /// components after it never get dropped, and the block never gets freed. The result is a
    /// memory leak rather than undefined behaviour. Accepting the leak is a decision, taken because
    /// a panicking clean-up is a bug in the component, and an uncaught panic on the main thread
    /// ends the program.
    ///
    /// # Examples
    ///
    /// The test `drop_with_varying_counts_drops_each_component_once` drops vectors of several
    /// lengths, and counts one drop for each component.
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
// from several threads at the same time.
//
// Changing a component or the vector requires `&mut`, or an `unsafe` write through a raw pointer,
// whose caller MUST guarantee that nothing else accesses that component. `Query::find_columns` for
// `&mut T` makes such a write possible through a shared `&Archetype`, by building a mutable slice
// from the raw pointer `data`. `Sync` therefore also relies on `&mut T` never implementing
// `ReadOnlyQuery`, which the seal on that trait ensures. `World::query` and
// `World::query_archetype` take `&World`, which other threads can share, and they accept only a
// `ReadOnlyQuery`. A `&mut T` that implemented `ReadOnlyQuery` would let two threads build mutable
// slices of one column at once, which is undefined behaviour
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
/// `ComponentInfo::of::<f32>(ComponentMask::single(0))` builds the `ComponentInfo` for `f32`, with
/// the bit at position 0. The test
/// `of_with_standard_and_extreme_types_returns_layout_and_drop_function` builds one for several
/// types.
#[derive(Clone, Copy, Debug)]
pub struct ComponentInfo {
    /// Identifies the component type.
    ///
    /// For the `ComponentInfo` in the [`ComponentInfo`] example, the field equals
    /// `TypeId::of::<f32>()`.
    type_id: TypeId,
    /// Stores the name of the component type, which a panic message reports.
    ///
    /// For the `ComponentInfo` in the [`ComponentInfo`] example, the field equals `"f32"`.
    type_name: &'static str,
    /// Defines the size and alignment of one component.
    ///
    /// For the `ComponentInfo` in the [`ComponentInfo`] example, the field has a size of 4 bytes and
    /// an alignment of 4.
    layout: Layout,
    /// Stores the function that runs the clean-up code of the component at the given address. The
    /// field is `None` where the type has no clean-up code.
    ///
    /// The field is `Some` for `String`, which frees its text, and `None` for `f32`.
    drop: Option<unsafe fn(*mut u8)>,
    /// Stores the mask with the one bit that stands for the component type, within one world.
    ///
    /// For the `ComponentInfo` in the [`ComponentInfo`] example, the field equals
    /// `ComponentMask::single(0)`.
    bit: ComponentMask,
}

impl ComponentInfo {
    /// Returns the [`ComponentInfo`] for the component type `T`, with `bit` as its bit in a table's
    /// mask.
    ///
    /// The caller MUST pass a mask with exactly one set bit, as [`ComponentMask::single`] builds,
    /// and a different bit for each component type in one world. The method stores `bit` without
    /// checking it. A wrong bit gives a table a wrong mask, and queries then match the wrong tables.
    ///
    /// The bound `T: 'static` means that `T` does not contain a borrowed reference that could expire. A
    /// vector can then keep a `T` for as long as it needs. `TypeId::of` requires the same bound.
    ///
    /// The bound `T: Send + Sync` lets a vector move its components between threads, and share them.
    ///
    /// The [`ComponentInfo`] example builds the `ComponentInfo` for `f32` with this method.
    pub(crate) fn of<T: Send + Sync + 'static>(bit: ComponentMask) -> Self {
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
    ///
    /// # Examples
    ///
    /// For the `ComponentInfo` of `f32`, `type_id` returns `TypeId::of::<f32>()`. The test
    /// `type_id_with_standard_and_extreme_types_returns_type_id_of_type` checks several types.
    pub(crate) fn type_id(self) -> TypeId {
        self.type_id
    }

    /// Returns the mask with the one bit that stands for the component type.
    ///
    /// # Examples
    ///
    /// For `ComponentInfo::of::<f32>(ComponentMask::single(2))`, `bit` returns
    /// `ComponentMask::single(2)`. The test
    /// `bit_with_standard_and_extreme_positions_returns_mask_from_of` checks several positions.
    pub(crate) fn bit(self) -> ComponentMask {
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
///
/// # Examples
///
/// Given a pointer to the bytes of a `String`, `drop_value::<String>` runs the clean-up code of
/// `String`, which frees its text. `ComponentInfo::of` stores it as the drop function for each type
/// with clean-up code. The test `drop_with_varying_counts_drops_each_component_once` reaches it
/// through the drop of a `ComponentVec`.
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
///
/// # Examples
///
/// For 3 components of 8 bytes, each aligned to 4, `array_layout` returns a layout of 24 bytes,
/// aligned to 4. The test `array_layout_with_standard_and_extreme_counts_returns_block_layout`
/// checks other counts.
fn array_layout(item: Layout, count: usize) -> Layout {
    let size = item
        .size()
        .checked_mul(count)
        .expect("the block size should fit in a `usize`");
    Layout::from_size_align(size, item.align())
        .expect("the block should be at most isize::MAX bytes")
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::Arc;
    use std::sync::Mutex;
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

    /// Adds its ID to a shared log when dropped, which shows which components had their clean-up
    /// code run.
    struct DropLog {
        id: u32,
        log: Arc<Mutex<Vec<u32>>>,
    }

    impl Drop for DropLog {
        fn drop(&mut self) {
            self.log.lock().unwrap().push(self.id);
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
            (
                "f32",
                ComponentInfo::of::<f32>(ComponentMask::single(0)),
                4,
                4,
                false,
            ),
            (
                "String",
                ComponentInfo::of::<String>(ComponentMask::single(0)),
                size_of::<String>(),
                align_of::<String>(),
                true,
            ),
            (
                "()",
                ComponentInfo::of::<()>(ComponentMask::single(0)),
                0,
                1,
                false,
            ),
            (
                "DropOnly",
                ComponentInfo::of::<DropOnly>(ComponentMask::single(0)),
                0,
                1,
                true,
            ),
            (
                "Aligned",
                ComponentInfo::of::<Aligned>(ComponentMask::single(0)),
                64,
                64,
                false,
            ),
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

        let health = ComponentInfo::of::<Health>(ComponentMask::single(0));
        let armour = ComponentInfo::of::<Armour>(ComponentMask::single(0));

        assert_eq!(health.layout, armour.layout, "layouts should match");
        assert_ne!(health.type_id, armour.type_id, "type IDs should differ");
    }

    /// Checks the starting state that a vector has before it allocates any memory
    #[test]
    fn new_with_standard_types_returns_empty_vector_with_aligned_pointer() {
        let cases = [
            ("u8", ComponentInfo::of::<u8>(ComponentMask::single(0))),
            ("f32", ComponentInfo::of::<f32>(ComponentMask::single(0))),
            ("u64", ComponentInfo::of::<u64>(ComponentMask::single(0))),
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

    /// Checks that a vector with no block of memory still returns a slice, which is empty
    #[test]
    fn as_slice_with_empty_vector_returns_empty_slice() {
        let components = ComponentVec::new(ComponentInfo::of::<f32>(ComponentMask::single(0)));
        let slice = components.as_slice::<f32>();

        assert!(slice.is_empty(), "slice should have no components");
    }

    /// Checks that a component survives being stored as bytes and read back as its own type
    #[test]
    fn push_typed_with_one_value_stores_value_for_as_slice() {
        let mut components = ComponentVec::new(ComponentInfo::of::<f32>(ComponentMask::single(0)));

        components.push_typed(1.5_f32);

        assert_eq!(components.as_slice::<f32>(), [1.5]);
    }

    /// Checks that `push` stores a `String` that the vector then owns, and reads it back unchanged
    #[test]
    fn push_with_manually_dropped_string_stores_value_for_as_slice() {
        let mut components =
            ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));
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
        let components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
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
            let mut components =
                ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

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
        let mut components =
            ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));

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
            let mut components =
                ComponentVec::new(ComponentInfo::of::<DropCounter>(ComponentMask::single(0)));
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
        let mut components =
            ComponentVec::new(ComponentInfo::of::<DropCounter>(ComponentMask::single(0)));

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
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        source.push_typed(10_u32);
        source.push_typed(20_u32);
        source.push_typed(30_u32);
        source.push_typed(40_u32);

        let mut target = ComponentVec::new(source.info());

        source.move_component(1, &mut target);

        assert_eq!(
            source.as_slice::<u32>(),
            [10, 40, 30],
            "source after the move"
        );
        assert_eq!(target.as_slice::<u32>(), [20], "target after the move");
    }

    /// Checks that moving the final component only pops it, since the move does not leave a
    /// gap to fill
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
            let mut source = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
            for &value in values {
                source.push_typed(value);
            }

            let mut target = ComponentVec::new(source.info());

            source.move_component(index, &mut target);

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
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        source.push_typed(50_u32);

        let mut target = ComponentVec::new(source.info());
        for value in [10_u32, 20, 30, 40] {
            target.push_typed(value);
        }

        source.move_component(0, &mut target);

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
        let mut source =
            ComponentVec::new(ComponentInfo::of::<DropCounter>(ComponentMask::single(0)));
        for _ in 0..5 {
            source.push_typed(DropCounter {
                drops: Arc::clone(&drops),
            });
        }

        let mut target = ComponentVec::new(source.info());

        source.move_component(1, &mut target); // fills the gap with the last component
        source.move_component(3, &mut target); // moves the last component, which does not leave a gap
        let drops_after_moves = drops.load(Ordering::Relaxed);

        drop(source);
        drop(target);
        let drops_after_vectors = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_moves, 0, "drops caused by the moves");
        assert_eq!(drops_after_vectors, 5, "drops after both vectors are gone");
    }

    /// Checks that a type of size zero is still counted, and never needs a block
    #[test]
    fn push_typed_with_zero_size_type_counts_without_allocating() {
        let mut components = ComponentVec::new(ComponentInfo::of::<()>(ComponentMask::single(0)));
        let start = components.data();

        for _ in 0..5 {
            components.push_typed(());
        }

        assert_eq!(components.as_slice::<()>().len(), 5, "number of components");
        assert_eq!(components.capacity, usize::MAX, "capacity");
        assert_eq!(components.data(), start, "pointer to the block");
    }

    /// Checks that a type of size zero still gets a pointer that meets its alignment
    #[test]
    fn as_slice_with_aligned_zero_size_type_returns_aligned_slice() {
        // Defines a type of size zero with an alignment that most addresses fail
        #[repr(align(64))]
        struct Tag;

        let mut components = ComponentVec::new(ComponentInfo::of::<Tag>(ComponentMask::single(0)));
        for _ in 0..3 {
            components.push_typed(Tag);
        }

        let slice = components.as_slice::<Tag>();
        let remainder = slice.as_ptr().addr() % 64;

        assert_eq!(slice.len(), 3, "number of components");
        assert_eq!(remainder, 0, "remainder of the address divided by 64");
    }

    /// Checks that a type of size zero still has its clean-up code run for each component
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

        let mut components =
            ComponentVec::new(ComponentInfo::of::<Marker>(ComponentMask::single(0)));
        for _ in 0..3 {
            components.push_typed(Marker);
        }

        drop(components);

        assert_eq!(DROPS.load(Ordering::Relaxed), 3);
    }

    /// Checks that a panic in the clean-up code of one component stops the drop at that component,
    /// and leaks every component after it
    #[test]
    #[cfg_attr(miri, ignore)]
    fn drop_with_panicking_drop_leaks_later_components() {
        // Defines a component whose clean-up code counts the drop, then panics where `panics` is set
        struct Fragile {
            panics: bool,
            drops: Arc<AtomicUsize>,
        }

        impl Drop for Fragile {
            fn drop(&mut self) {
                self.drops.fetch_add(1, Ordering::Relaxed);
                assert!(!self.panics, "the clean-up code of the component panicked");
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let mut components =
            ComponentVec::new(ComponentInfo::of::<Fragile>(ComponentMask::single(0)));
        for panics in [false, true, false, false] {
            components.push_typed(Fragile {
                panics,
                drops: Arc::clone(&drops),
            });
        }

        let result = panic::catch_unwind(AssertUnwindSafe(|| drop(components)));
        let drops_after_panic = drops.load(Ordering::Relaxed);
        // Counts the owners of the counter, which include each leaked component
        let owners_after_panic = Arc::strong_count(&drops);

        assert!(result.is_err(), "drop should pass the panic on");
        assert_eq!(drops_after_panic, 2, "drops up to the panicking component");
        assert_eq!(owners_after_panic, 3, "owners of the counter, two leaked");
    }

    /// Checks that a type of size zero moves by count alone, and is cleaned up once per
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

        let mut source = ComponentVec::new(ComponentInfo::of::<Marker>(ComponentMask::single(0)));
        for _ in 0..3 {
            source.push_typed(Marker);
        }

        let mut target = ComponentVec::new(source.info());

        source.move_component(0, &mut target);

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

    /// Checks that a type with a large alignment stays aligned at every index of the target, after
    /// a move into a full target that has to grow
    #[test]
    fn move_component_with_large_alignment_keeps_every_component_aligned() {
        #[repr(align(64))]
        struct Aligned(u8);

        let mut source = ComponentVec::new(ComponentInfo::of::<Aligned>(ComponentMask::single(0)));
        for value in 0..3_u8 {
            source.push_typed(Aligned(value));
        }
        let mut target = ComponentVec::new(source.info());
        for value in 10..14_u8 {
            target.push_typed(Aligned(value));
        }

        // Moves the middle component into the full target, which makes the target grow from 4 to 8
        source.move_component(1, &mut target);

        let remainders: Vec<usize> = (0..target.len())
            .map(|index| target.ptr_at(index).addr() % 64)
            .collect();
        let source_values: Vec<u8> = source
            .as_slice::<Aligned>()
            .iter()
            .map(|component| component.0)
            .collect();
        let target_values: Vec<u8> = target
            .as_slice::<Aligned>()
            .iter()
            .map(|component| component.0)
            .collect();
        assert_eq!(
            remainders, [0; 5],
            "remainder of each target address divided by 64"
        );
        assert_eq!(source_values, [0, 2], "values left in the source");
        assert_eq!(target_values, [10, 11, 12, 13, 1], "values in the target");
    }

    /// Checks that a vector for a type of size zero stops at its limit, rather than allocating
    #[test]
    #[should_panic]
    fn push_typed_with_full_zero_size_vector_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<()>(ComponentMask::single(0)));

        // Sets the length by hand, since pushing `usize::MAX` components would take too long
        components.len = usize::MAX;

        components.push_typed(());
    }

    /// Checks that a type with a large alignment stays aligned at every index as the block grows
    #[test]
    fn push_typed_with_large_alignment_keeps_every_component_aligned() {
        #[repr(align(64))]
        struct Aligned(u8);

        let mut components =
            ComponentVec::new(ComponentInfo::of::<Aligned>(ComponentMask::single(0)));

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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

        components.push_typed(1.5_f32);
    }

    /// Checks that reading the stored bytes as another type panics, even at the same size
    #[test]
    #[should_panic]
    fn as_slice_with_wrong_type_panics() {
        let components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

        components.as_slice::<f32>();
    }

    /// Checks that changing the stored bytes as another type panics, even at the same size
    #[test]
    #[should_panic]
    fn as_mut_slice_with_wrong_type_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

        components.as_mut_slice::<f32>();
    }

    /// Checks that every build catches an index that would read past the last component
    #[test]
    #[should_panic]
    fn move_component_with_out_of_range_index_panics() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(source.info());

        source.move_component(1, &mut target);
    }

    /// Checks that every build catches a move between columns of different types
    #[test]
    #[should_panic]
    fn move_component_with_mismatched_target_panics() {
        let mut source = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        source.push_typed(10_u32);

        let mut target = ComponentVec::new(ComponentInfo::of::<f32>(ComponentMask::single(0)));

        source.move_component(0, &mut target);
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
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
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
        let mut source_health =
            ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        let mut source_names =
            ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));
        for (health, name) in [(10_u32, "Ada"), (20, "Bo"), (30, "Cy")] {
            source_health.push_typed(health);
            source_names.push_typed(String::from(name));
        }

        let mut target_health = ComponentVec::new(source_health.info());
        let mut target_names = ComponentVec::new(source_names.info());

        source_health.move_component(0, &mut target_health);
        source_names.move_component(0, &mut target_names);

        assert_eq!(source_health.as_slice::<u32>(), [30, 20], "source health");
        assert_eq!(
            source_names.as_slice::<String>(),
            ["Cy", "Bo"],
            "source names"
        );
        assert_eq!(target_health.as_slice::<u32>(), [10], "target health");
        assert_eq!(target_names.as_slice::<String>(), ["Ada"], "target names");
    }

    /// Checks that `type_id` returns the `TypeId` of the type that `of` was called with
    #[test]
    fn type_id_with_standard_and_extreme_types_returns_type_id_of_type() {
        // Pairs each `ComponentInfo` with the `TypeId` that it should return
        let cases = [
            (
                "f32",
                ComponentInfo::of::<f32>(ComponentMask::single(0)),
                TypeId::of::<f32>(),
            ),
            (
                "String",
                ComponentInfo::of::<String>(ComponentMask::single(0)),
                TypeId::of::<String>(),
            ),
            (
                "()",
                ComponentInfo::of::<()>(ComponentMask::single(0)),
                TypeId::of::<()>(),
            ),
        ];

        for (name, info, expected) in cases {
            let type_id = info.type_id();

            assert_eq!(type_id, expected, "type ID of {name}");
        }
    }

    /// Checks that `bit` returns the mask that `of` received, up to the last position in a mask
    #[test]
    fn bit_with_standard_and_extreme_positions_returns_mask_from_of() {
        let cases = [0, 5, ComponentMask::CAPACITY - 1];

        for position in cases {
            let mask = ComponentMask::single(position);
            let info = ComponentInfo::of::<u32>(mask);

            let bit = info.bit();

            assert_eq!(bit, mask, "bit at position {position}");
        }
    }

    /// Checks that a vector returns the `ComponentInfo` that it was created with
    #[test]
    fn info_with_standard_type_returns_info_from_new() {
        let info = ComponentInfo::of::<f32>(ComponentMask::single(3));
        let components = ComponentVec::new(info);

        let returned = components.info();

        assert_eq!(returned.type_id(), TypeId::of::<f32>(), "type ID");
        assert_eq!(returned.bit(), ComponentMask::single(3), "bit");
        assert_eq!(returned.layout, info.layout, "layout");
    }

    /// Checks that the length counts the components in the vector, before and after the block grows
    #[test]
    fn len_with_varying_counts_returns_number_of_components() {
        let cases = [0_u32, 1, 4, 5];

        for count in cases {
            let mut components =
                ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
            for value in 0..count {
                components.push_typed(value);
            }

            let len = components.len();

            assert_eq!(len, count as usize, "length after {count} pushes");
        }
    }

    /// Checks that `data` points at index 0 once the vector has a block, where the first component
    /// lies
    #[test]
    fn data_with_pushed_components_returns_address_of_first_component() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        components.push_typed(7_u32);
        components.push_typed(8_u32);

        let data = components.data();
        // SAFETY: `read` requires an aligned pointer to a live `u32`. `data` is always aligned, and
        // index 0 stores the live component 7. `u32` is `Copy`, meaning the copy has no clean-up code
        let first = unsafe { data.cast::<u32>().read() };

        assert_eq!(data.as_ptr(), components.ptr_at(0), "address of index 0");
        assert_eq!(first, 7, "component at the address");
    }

    /// Checks that a removal from any index but the last fills the gap with the last component
    #[test]
    fn swap_remove_with_non_last_index_fills_gap_with_last_component() {
        // Lists the starting components, the index to remove, and the components that should remain
        let cases: [(&[u32], usize, &[u32]); 3] = [
            (&[10, 20, 30, 40], 0, &[40, 20, 30]),
            (&[10, 20, 30, 40], 1, &[10, 40, 30]),
            (&[10, 20], 0, &[20]),
        ];

        for (values, index, remaining) in cases {
            let mut components =
                ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
            for &value in values {
                components.push_typed(value);
            }

            components.swap_remove(index);

            assert_eq!(
                components.as_slice::<u32>(),
                remaining,
                "components after removing index {index} from {values:?}"
            );
        }
    }

    /// Checks that removing the final component only pops it, since the removal does not leave a gap
    /// to fill
    #[test]
    fn swap_remove_with_last_index_leaves_other_components_in_place() {
        // Lists the starting components, the index to remove, and the components that should remain
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
            let mut components =
                ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
            for &value in values {
                components.push_typed(value);
            }

            components.swap_remove(index);

            assert_eq!(
                components.as_slice::<u32>(),
                remaining,
                "components after removing from {values:?}"
            );
        }
    }

    /// Checks that a removal runs the clean-up code of the removed component alone, and that the
    /// vector later drops each remaining component once
    #[test]
    fn swap_remove_with_drop_logs_drops_only_removed_component() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut components =
            ComponentVec::new(ComponentInfo::of::<DropLog>(ComponentMask::single(0)));
        for id in 0..5 {
            components.push_typed(DropLog {
                id,
                log: Arc::clone(&log),
            });
        }

        components.swap_remove(1);
        let dropped_by_removal = log.lock().unwrap().clone();
        let remaining: Vec<u32> = components
            .as_slice::<DropLog>()
            .iter()
            .map(|component| component.id)
            .collect();

        drop(components);
        let drops_after_vector = log.lock().unwrap().len();

        assert_eq!(dropped_by_removal, [1], "IDs dropped by the removal");
        assert_eq!(remaining, [0, 4, 2, 3], "IDs left in the vector");
        assert_eq!(drops_after_vector, 5, "drops after the vector is gone");
    }

    /// Checks that heap memory owned by the components that remain stays reachable after a removal
    #[test]
    fn swap_remove_with_strings_keeps_remaining_strings_readable() {
        let mut components =
            ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));
        for name in ["Ada", "Bo", "Cy", "Dee", "Eli"] {
            components.push_typed(String::from(name));
        }

        components.swap_remove(0);
        components.swap_remove(3);

        assert_eq!(components.as_slice::<String>(), ["Eli", "Bo", "Cy"]);
    }

    /// Checks that a type of size zero is removed by count alone, and is cleaned up once per
    /// component
    #[test]
    fn swap_remove_with_zero_size_drop_type_drops_each_component_once() {
        // Counts the drops outside the type, since a type of size zero has no room for a counter
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Marker;
        impl Drop for Marker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        let mut components =
            ComponentVec::new(ComponentInfo::of::<Marker>(ComponentMask::single(0)));
        for _ in 0..3 {
            components.push_typed(Marker);
        }

        components.swap_remove(0);
        let len_after_removal = components.len();
        let drops_after_removal = DROPS.load(Ordering::Relaxed);

        drop(components);
        let drops_after_vector = DROPS.load(Ordering::Relaxed);

        assert_eq!(len_after_removal, 2, "components left after the removal");
        assert_eq!(drops_after_removal, 1, "drops caused by the removal");
        assert_eq!(drops_after_vector, 3, "drops after the vector is gone");
    }

    /// Checks that a vector stays usable after the clean-up code of a removed component panics, and
    /// that it never drops that component again
    #[test]
    fn swap_remove_with_panicking_drop_leaves_vector_usable() {
        // Defines a component whose clean-up code counts the drop, then panics where `panics` is set
        struct Fragile {
            id: u32,
            panics: bool,
            drops: Arc<AtomicUsize>,
        }

        impl Drop for Fragile {
            fn drop(&mut self) {
                self.drops.fetch_add(1, Ordering::Relaxed);
                assert!(!self.panics, "the clean-up code of the component panicked");
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let mut components =
            ComponentVec::new(ComponentInfo::of::<Fragile>(ComponentMask::single(0)));
        for (id, panics) in [(0, true), (1, false), (2, false)] {
            components.push_typed(Fragile {
                id,
                panics,
                drops: Arc::clone(&drops),
            });
        }

        // Catches the panic from the clean-up code of index 0, which `swap_remove` passes on
        let result = panic::catch_unwind(AssertUnwindSafe(|| components.swap_remove(0)));
        let len_after_panic = components.len();
        components.push_typed(Fragile {
            id: 3,
            panics: false,
            drops: Arc::clone(&drops),
        });
        let ids: Vec<u32> = components
            .as_slice::<Fragile>()
            .iter()
            .map(|component| component.id)
            .collect();

        drop(components);
        let drops_after_vector = drops.load(Ordering::Relaxed);

        assert!(result.is_err(), "swap_remove should pass the panic on");
        assert_eq!(len_after_panic, 2, "length after the panic");
        assert_eq!(ids, [2, 1, 3], "IDs in the vector after a later push");
        assert_eq!(drops_after_vector, 4, "drops after the vector is gone");
    }

    /// Checks that every build catches an index that would remove a component past the last one
    #[test]
    #[should_panic]
    fn swap_remove_with_out_of_range_index_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        components.push_typed(10_u32);

        components.swap_remove(1);
    }

    /// Checks that a removal from an empty vector panics, rather than reaching an index below zero
    #[test]
    #[should_panic]
    fn swap_remove_with_empty_vector_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));

        components.swap_remove(0);
    }

    /// Checks that the new capacity is the larger of the capacity needed and double the old one, and
    /// that a block with room already stays the same size
    #[test]
    fn reserve_with_standard_and_extreme_amounts_returns_expected_capacity() {
        // Lists the number of pushes, the amount to reserve, and the capacity that should result
        let cases = [
            (0_u32, 0, 0),
            (0, 1, 1),
            (0, 10, 10),
            (3, 1, 4),
            (4, 1, 8),
            (4, 10, 14),
            (5, 3, 8),
        ];

        for (count, additional, capacity) in cases {
            let mut components =
                ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
            for value in 0..count {
                components.push_typed(value);
            }

            components.reserve(additional);

            assert_eq!(
                components.capacity, capacity,
                "capacity after {count} pushes and a reserve of {additional}"
            );
        }
    }

    /// Checks that a reserve within the room that the block has keeps the block at its address
    #[test]
    fn reserve_with_room_available_keeps_block() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        components.push_typed(1_u32);
        let start = components.data();

        components.reserve(3);

        assert_eq!(components.data(), start, "pointer to the block");
    }

    /// Checks that the pushes that a reserve makes room for fit in the block without moving it
    #[test]
    fn reserve_with_later_pushes_keeps_block_in_place() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        components.reserve(10);
        let start = components.data();

        for value in 0..10_u32 {
            components.push_typed(value);
        }

        assert_eq!(components.capacity, 10, "capacity after the pushes");
        assert_eq!(components.data(), start, "pointer to the block");
    }

    /// Checks that heap memory owned by a component stays reachable after a reserve moves the block
    #[test]
    fn reserve_with_growth_keeps_strings_readable() {
        let names = ["Ada", "Bo", "Cy"];
        let mut components =
            ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));
        for name in names {
            components.push_typed(String::from(name));
        }

        components.reserve(10);

        assert_eq!(components.as_slice::<String>(), names);
    }

    /// Checks that a vector for a type of size zero always has the room, and keeps its block
    #[test]
    fn reserve_with_zero_size_type_keeps_capacity_and_block() {
        let cases = [0, 1, usize::MAX];

        for additional in cases {
            let mut components =
                ComponentVec::new(ComponentInfo::of::<()>(ComponentMask::single(0)));
            let start = components.data();

            components.reserve(additional);

            assert_eq!(
                components.capacity,
                usize::MAX,
                "capacity after a reserve of {additional}"
            );
            assert_eq!(
                components.data(),
                start,
                "pointer after a reserve of {additional}"
            );
        }
    }

    /// Checks the limit when the length plus the amount to reserve is too large for `usize`
    #[test]
    #[should_panic]
    fn reserve_with_overflowing_count_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u32>(ComponentMask::single(0)));
        components.push_typed(1_u32);

        components.reserve(usize::MAX);
    }

    /// Checks the limit when the block would pass the largest size allowed, before anything is
    /// allocated
    #[test]
    #[should_panic]
    fn reserve_with_size_above_limit_panics() {
        let mut components = ComponentVec::new(ComponentInfo::of::<u8>(ComponentMask::single(0)));
        let additional = isize::MAX as usize + 1;

        components.reserve(additional);
    }

    /// Checks that heap memory owned by the components stays reachable when a move grows the target
    #[test]
    fn move_component_with_strings_and_full_target_keeps_strings_readable() {
        let mut source = ComponentVec::new(ComponentInfo::of::<String>(ComponentMask::single(0)));
        for name in ["Ada", "Bo", "Cy"] {
            source.push_typed(String::from(name));
        }

        let mut target = ComponentVec::new(source.info());
        for name in ["Dee", "Eli", "Fay", "Gus"] {
            target.push_typed(String::from(name));
        }

        source.move_component(0, &mut target);

        assert_eq!(source.as_slice::<String>(), ["Cy", "Bo"], "source");
        assert_eq!(
            target.as_slice::<String>(),
            ["Dee", "Eli", "Fay", "Gus", "Ada"],
            "target"
        );
    }

    /// Checks that each component is dropped once after a mix of reserves, pushes, removals and moves
    #[test]
    fn drop_with_mixed_operations_drops_each_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut source =
            ComponentVec::new(ComponentInfo::of::<DropCounter>(ComponentMask::single(0)));
        let mut target = ComponentVec::new(source.info());

        // Reserves room for 5, then pushes 7, which makes the block grow from 5 to 10
        source.reserve(5);
        for _ in 0..7 {
            source.push_typed(DropCounter {
                drops: Arc::clone(&drops),
            });
        }
        source.swap_remove(2);
        source.move_component(0, &mut target);
        source.move_component(4, &mut target);
        target.reserve(10);
        let drops_before_vectors = drops.load(Ordering::Relaxed);
        let lengths = (source.len(), target.len());

        drop(source);
        drop(target);
        let drops_after_vectors = drops.load(Ordering::Relaxed);

        assert_eq!(drops_before_vectors, 1, "drops caused by the removal");
        assert_eq!(lengths, (4, 2), "lengths of the source and the target");
        assert_eq!(drops_after_vectors, 7, "drops after both vectors are gone");
    }
}
