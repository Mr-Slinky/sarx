//! Defines [`Query`], the trait that describes which component types a caller visits on each
//! entity. A query also states whether the caller reads or changes each of those types.
//!
//! A caller writes a query as a type. `&T` reads the `T` of each entity, and `&mut T` changes it. A
//! tuple such as `(&mut Position, &Velocity, &Health)` asks for every type in it at once, and can
//! contain up to thirty-two queries.
//!
//! The caller passes the query type to `World::query_mut`, which returns one item for each entity
//! that has every type in the query. A query that only reads implements [`ReadOnlyQuery`]. The
//! caller can pass such a query to `World::query` instead, which borrows the world through a shared
//! reference. The documentation on each of the two methods shows a full example.
//!
//! The example on [`Query`] passes the query `(&mut Position, &Velocity)` to a function that
//! accepts any query type.
//!
//! # Rules on permissions
//!
//! A query changes a component type only where it asks for that type once. A query such as
//! `(&mut Position, &Position)` would hand out two references to the same `Position`, one of them
//! able to change it. Rust forbids that pair of references. [`check_permissions`] therefore panics
//! on such a query, before the world hands out any reference.

use crate::Component;
use crate::archetype::Archetype;
use std::any::TypeId;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::slice;

// ========================================================================================== \\
//                                          Modules                                           \\
// ========================================================================================== \\

/// Declares the traits that seal [`Query`] and [`ReadOnlyQuery`].
///
/// The example on [`Query`] passes the query `(&mut Position, &Velocity)`, one of the types that
/// the crate implements `Sealed` for.
mod sealed {
    /// Marks each type that the crate implements [`Query`](super::Query) for.
    ///
    /// The trait is `pub` inside a private module. Code outside the crate therefore cannot write a
    /// path to it, and cannot implement it.
    ///
    /// The example on [`Query`](super::Query) passes the query `(&mut Position, &Velocity)`, one of
    /// the types that implement this trait.
    pub trait Sealed {}

    /// Marks each type that the crate implements [`ReadOnlyQuery`](super::ReadOnlyQuery) for.
    ///
    /// The trait is `pub` inside a private module, as `Sealed` is. `Sealed` cannot seal
    /// `ReadOnlyQuery` on its own, since `&mut T` implements `Sealed` as a `Query`. This trait
    /// leaves `&mut T` out.
    ///
    /// The first example on [`ReadOnlyQuery`](super::ReadOnlyQuery) passes `&Position`, one of the
    /// types that implement this trait.
    pub trait ReadOnlySealed {}
}

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Represents whether a query reads a component type or changes it.
///
/// The test `add_permissions_with_each_query_shape_pushes_entries_in_order` lists the `Permission`
/// that each query shape records. `&Position` records `Permission::Read`, and `&mut Position`
/// records `Permission::Write`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// The query reads the component, through a `&T`.
    ///
    /// The test `add_permissions_with_each_query_shape_pushes_entries_in_order` shows `&Position`
    /// recording `Permission::Read`.
    Read,
    /// The query changes the component, through a `&mut T`.
    ///
    /// The test `add_permissions_with_each_query_shape_pushes_entries_in_order` shows
    /// `&mut Position` recording `Permission::Write`.
    Write,
}

/// Describes the component types that a query asks for, and how the world hands them out.
///
/// A query type is `&T` for a component that the caller reads, or `&mut T` for one that it changes.
/// A tuple of one to thirty-two query types, such as `(A, B, C)`, is also a query, and asks for
/// every type that its fields ask for.
///
/// The world runs a query in three steps. It first calls [`Query::add_permissions`] to collect the
/// component types that the query asks for. It then calls [`Query::find_columns`] to borrow the
/// matching columns of each table. Finally, it calls [`Query::split_rows`], which reads each row
/// through [`Query::fetch_row`], to split those columns into one item per entity.
/// `World::query_archetype_mut` and `World::query_archetype` stop after the second step, and
/// return the columns.
///
/// # Lifetimes
///
/// The lifetime `'w` stands for the borrow of the world. Every column and every item borrows from
/// the world for `'w`. As a result, the compiler rejects any other use of the world while a column
/// or an item is in use.
///
/// # Visibility
///
/// Code outside the crate can name the trait as a bound, and can pass any query type that the
/// crate implements it for. The attribute `#[doc(hidden)]` hides the three methods from the public
/// documentation, since only code inside the crate needs them.
///
/// Code outside the crate cannot implement the trait. `Query` requires the trait `Sealed`, which a
/// private module in `query.rs` declares. Code outside the crate cannot write a path to `Sealed`,
/// and therefore cannot implement it.
///
/// # Safety
///
/// An implementation MUST guarantee the following:
///
/// - [`Query::add_permissions`] pushes one entry for every component type that
///   [`Query::find_columns`] borrows. The entry is `Permission::Write` for every type that
///   `find_columns` borrows mutably.
/// - [`Query::find_columns`] borrows only the columns of the table that it receives.
///
/// `check_permissions` relies on the first guarantee to reject a query that borrows one column
/// mutably and also borrows it a second time. A missing entry would let a mutable reference reach a
/// component that another reference also reaches, which is undefined behaviour.
///
/// # Examples
///
/// The example defines a generic function that accepts any query type. It then calls that function
/// with the query `(&mut Position, &Velocity)`, which changes each `Position` and reads each
/// `Velocity`.
///
/// ```
/// use enki::Query;
/// # use enki::Component;
///
/// struct Position(f32, f32);
/// struct Velocity(f32, f32);
/// # impl Component for Position {}
/// # impl Component for Velocity {}
///
/// // Accepts any query type
/// fn accept<Q: Query>() {}
///
/// // Passes a query that changes each `Position` and reads each `Velocity`
/// accept::<(&mut Position, &Velocity)>();
/// ```
pub unsafe trait Query: sealed::Sealed {
    /// Represents the columns that the query borrows from one table, for the lifetime `'w`.
    ///
    /// `&T` borrows its column as a `&'w [T]`, and `&mut T` as a `&'w mut [T]`. A tuple borrows the
    /// columns that each of its fields borrows.
    ///
    /// The example on [`World::query_archetype_mut`](crate::World::query_archetype_mut) receives
    /// the columns of `(&mut Position, &Velocity)` as a `(&mut [Position], &[Velocity])`.
    type Columns<'w>;

    /// Represents the item that the query produces for one entity, for the lifetime `'w`.
    ///
    /// `&T` produces a `&'w T`, and `&mut T` a `&'w mut T`. A tuple produces a tuple that contains
    /// the item that each field produces.
    ///
    /// The example on [`World::query_mut`](crate::World::query_mut) receives the item of
    /// `(&mut Position, &Velocity)` as a `(&mut Position, &Velocity)`.
    type Item<'w>;

    /// Pushes one entry onto `permissions` for each component type that the query asks for.
    ///
    /// Each entry pairs the [`TypeId`] of the type with the `Permission` that the query needs. A
    /// tuple pushes the entries of each field, in field order. The method leaves every existing
    /// entry in `permissions` in place.
    ///
    /// The test `add_permissions_with_each_query_shape_pushes_entries_in_order` lists the entries
    /// that single types, tuples and tuples inside tuples push.
    #[doc(hidden)]
    fn add_permissions(permissions: &mut Vec<(TypeId, Permission)>);

    /// Returns the columns in `archetype` that the query asks for, or `None` where the table lacks
    /// one of the component types.
    ///
    /// The columns borrow `archetype` for the lifetime `'w`. A column that the query changes comes
    /// back as a mutable slice, although the method receives a shared reference to the table.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee three things:
    ///
    /// - Nothing changes the columns of `archetype` during `'w`, apart from the query itself
    ///   through a column that it borrows mutably. Nothing else reads such a column. A caller that
    ///   holds `&mut World` for `'w` meets this guarantee. So does a caller that holds `&World` for
    ///   a query that implements [`ReadOnlyQuery`].
    /// - `check_permissions` has already returned for the query type, without a panic.
    /// - No other live slice of a column that the query borrows mutably exists during `'w`. Shared
    ///   slices of one column can overlap, which Rust allows. Two `query::<&Position>()` iterators
    ///   over one `&World` build two such slices, and so does the query `(&Position, &Position)`.
    ///
    /// A broken guarantee lets a mutable slice reach a component that another reference also
    /// reaches, which is undefined behaviour.
    ///
    /// # Examples
    ///
    /// The test `find_columns_with_read_query_returns_column_in_row_order` builds a table with
    /// three rows, and checks that `&Position` returns the `Position` column in row order.
    #[doc(hidden)]
    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>>;

    /// Represents the columns of one table in the form that [`Query::fetch_row`] reads one row
    /// from.
    ///
    /// `&T` keeps its `&'w [T]`. `&mut T` keeps a [`RowsMut`], which points at the first component
    /// of the column and records the row count. A tuple keeps the rows of each field.
    ///
    /// The test `split_rows_with_pair_zips_items_by_row` builds the rows of
    /// `(&Position, &Velocity)` through [`Query::split_rows`].
    #[doc(hidden)]
    type Rows<'w>;

    /// Returns `columns` in the form that [`Query::fetch_row`] reads one row from.
    ///
    /// The rows borrow the table for the lifetime `'w`, as the columns did.
    ///
    /// The test `split_rows_with_write_query_changes_reach_table` turns the columns of
    /// `(&mut Position, &Velocity)` into rows through [`Query::split_rows`].
    #[doc(hidden)]
    fn build_rows<'w>(columns: Self::Columns<'w>) -> Self::Rows<'w>;

    /// Returns the number of rows that [`Query::fetch_row`] can read from `rows`.
    ///
    /// A tuple returns the smallest count among its fields. Every column of a table has one
    /// component per entity, meaning the counts are equal in practice.
    ///
    /// The test `split_rows_with_empty_table_yields_no_items` counts zero rows for three queries.
    #[doc(hidden)]
    fn count_rows(rows: &Self::Rows<'_>) -> usize;

    /// Returns the item for the entity at `row`.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee two things:
    ///
    /// - `row` is below [`Query::count_rows`] for `rows`.
    /// - The caller reads each row at most once from one `Rows` value. A second read of a row would
    ///   hand out a second `&mut T` to a component that the first one already reaches, which is
    ///   undefined behaviour.
    ///
    /// # Examples
    ///
    /// For the rows of a `Position` column that holds `Position(1.0)` and `Position(2.0)`, row 1
    /// returns `&Position(2.0)`. The test `split_rows_with_read_query_yields_items_in_row_order`
    /// reads every row in order.
    #[doc(hidden)]
    unsafe fn fetch_row<'w>(rows: &Self::Rows<'w>, row: usize) -> Self::Item<'w>;

    /// Returns an iterator over the rows in `columns`, with one item for each entity in the table.
    ///
    /// The items come in row order. The item for each entity borrows from the columns for the
    /// lifetime `'w`.
    ///
    /// The iterator counts the rows once, then reads row 0, row 1 and so on through
    /// [`Query::fetch_row`]. A tuple therefore checks one row counter per item, however many fields
    /// it has.
    ///
    /// The test `split_rows_with_read_query_yields_items_in_row_order` splits a `Position` column
    /// of three rows, and checks that the items come in row order.
    #[doc(hidden)]
    fn split_rows<'w>(columns: Self::Columns<'w>) -> impl Iterator<Item = Self::Item<'w>> {
        let rows = Self::build_rows(columns);
        let count = Self::count_rows(&rows);
        // SAFETY: `fetch_row` requires a row below `count_rows`, read at most once. The range
        // yields each row below `count` once, and the closure owns `rows`, meaning nothing else
        // reads from it
        (0..count).map(move |row| unsafe { Self::fetch_row(&rows, row) })
    }
}

/// Points at the column of one table that a `&mut T` query changes, one row at a time.
///
/// [`Query::fetch_row`] for `&mut T` builds each `&mut T` from `start`, the pointer to the first
/// component. Each row then reaches its component without a fresh borrow of the whole column. A
/// fresh `&mut` borrow of the whole column, made once per row, would end the borrows of the rows
/// already handed out.
///
/// # Lifetimes
///
/// The lifetime `'w` is the borrow of the column that [`Query::build_rows`] receives as a
/// `&'w mut [T]`. `start` is a raw pointer, which carries no lifetime. The field `borrow` keeps
/// `'w` attached, as though the struct still contained the `&'w mut [T]`. The compiler therefore
/// treats the column as borrowed for as long as the struct or any item built from it is in use.
///
/// # Visibility
///
/// The type is `pub`, since it is the `Rows` type of `&mut T`, which is a public type. The
/// `query` module is private, meaning code outside the crate cannot write a path to it.
///
/// The test `split_rows_with_write_query_changes_reach_table` changes components through one.
#[doc(hidden)]
pub struct RowsMut<'w, T> {
    /// Points at the component in row 0.
    start: NonNull<T>,
    /// Records the number of rows in the column.
    len: usize,
    /// Ties the struct to the borrow of the column, as the Lifetimes section sets out.
    borrow: PhantomData<&'w mut [T]>,
}

// SAFETY: `RowsMut` stands in for a `&'w mut [T]`, which a thread can send to another where `T` is
// `Send`. A raw pointer is never `Send` on its own, so the impl restores what the slice allowed
unsafe impl<T: Send> Send for RowsMut<'_, T> {}

// SAFETY: as for `Send`, a `&'w mut [T]` can be shared between threads where `T` is `Sync`
unsafe impl<T: Sync> Sync for RowsMut<'_, T> {}

impl<T: Component> sealed::Sealed for &T {}

// SAFETY: `add_permissions` pushes one `Permission::Read` entry for `T`, which is the one type that
// `find_columns` borrows. `find_columns` borrows only the `T` column of the table that it receives
unsafe impl<T: Component> Query for &T {
    type Columns<'w> = &'w [T];
    type Item<'w> = &'w T;

    fn add_permissions(permissions: &mut Vec<(TypeId, Permission)>) {
        permissions.push((TypeId::of::<T>(), Permission::Read));
    }

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
        let components = archetype.components_of(TypeId::of::<T>())?;
        Some(components.as_slice::<T>())
    }

    type Rows<'w> = &'w [T];

    fn build_rows<'w>(columns: Self::Columns<'w>) -> Self::Rows<'w> {
        columns
    }

    fn count_rows(rows: &Self::Rows<'_>) -> usize {
        rows.len()
    }

    unsafe fn fetch_row<'w>(rows: &Self::Rows<'w>, row: usize) -> Self::Item<'w> {
        // Reaches the slice behind the reference, which keeps the item tied to `'w` rather than to
        // the short borrow of `rows`
        let column: &'w [T] = rows;
        // SAFETY: `get_unchecked` requires an index below the length of the slice. The caller
        // guarantees that `row` is below `count_rows`, which is that length
        unsafe { column.get_unchecked(row) }
    }
}

impl<T: Component> sealed::Sealed for &mut T {}

// SAFETY: `add_permissions` pushes one `Permission::Write` entry for `T`, which is the one type
// that `find_columns` borrows, and borrows mutably. `find_columns` borrows only the `T` column of
// the table that it receives
unsafe impl<T: Component> Query for &mut T {
    type Columns<'w> = &'w mut [T];
    type Item<'w> = &'w mut T;

    fn add_permissions(permissions: &mut Vec<(TypeId, Permission)>) {
        permissions.push((TypeId::of::<T>(), Permission::Write));
    }

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
        let components = archetype.components_of(TypeId::of::<T>())?;
        let start = components.data().cast::<T>();
        let len = components.len();

        // SAFETY: `from_raw_parts_mut` requires an aligned pointer to `len` live values of `T`.
        // `components_of` returned the column for the `TypeId` of `T`, `data` is always aligned,
        // and every index below `len` stores a live component. The slice also has to span at most
        // `isize::MAX` bytes, and `array_layout` keeps every block within that size.
        //
        // `from_raw_parts_mut` also requires that nothing else reads or changes those components
        // during `'w`. A shared reference forbids changes only to the memory that it points at
        // directly. The components lie in a separate block, which the column reaches through the
        // raw pointer `data`. The borrow of `archetype` therefore leaves that block open to a write
        // through `start`. Three guarantees from the caller keep every other reference off it:
        //
        // - The caller holds `&mut World` for `'w`, meaning nothing outside the query reaches any
        //   table. A `&mut T` query reaches this method only through the `_mut` methods on
        //   `World`, since `&mut T` does not implement `ReadOnlyQuery`, and the seal on
        //   `ReadOnlyQuery` stops any other crate from implementing it.
        // - `check_permissions` has passed, meaning `T` appears once in the query. Every other part
        //   of the query borrows a column of another type, which has its own block.
        // - No other live slice of this column exists during `'w`, as the third guarantee in the
        //   Safety section of `find_columns` requires.
        Some(unsafe { slice::from_raw_parts_mut(start.as_ptr(), len) })
    }

    type Rows<'w> = RowsMut<'w, T>;

    fn build_rows<'w>(columns: Self::Columns<'w>) -> Self::Rows<'w> {
        let len = columns.len();
        RowsMut {
            start: NonNull::from(columns).cast::<T>(),
            len,
            borrow: PhantomData,
        }
    }

    fn count_rows(rows: &Self::Rows<'_>) -> usize {
        rows.len
    }

    unsafe fn fetch_row<'w>(rows: &Self::Rows<'w>, row: usize) -> Self::Item<'w> {
        // SAFETY: `add` requires the result to stay within the column, and the caller guarantees
        // that `row` is below `len`. `start` came from the `&'w mut [T]` that `build_rows`
        // consumed, meaning it may write to every component for `'w`, and each one is live. The
        // caller reads each row once, meaning no other `&mut` reaches the component at `row`
        unsafe { &mut *rows.start.as_ptr().add(row) }
    }
}

/// Marks a query that only reads its components.
///
/// `&T` implements the trait, and so does a tuple whose fields all implement it. `&mut T` does not
/// implement it, and neither does a tuple with a `&mut T` field. The trait declares no methods. It
/// requires [`Query`], which describes the component types that the query asks for.
///
/// [`World::query`](crate::World::query) and
/// [`World::query_archetype`](crate::World::query_archetype) accept only a query that implements
/// the trait. Both methods borrow the world through a shared reference. As a result, two read
/// queries can run at once, and a loop over one query can call `World::get`.
///
/// # Visibility
///
/// Code outside the crate can name the trait as a bound, and can pass any query type that the
/// crate implements it for. Code outside the crate cannot implement the trait. `ReadOnlyQuery`
/// requires the trait `ReadOnlySealed`, which a private module in `query.rs` declares. Code outside
/// the crate cannot write a path to `ReadOnlySealed`, and therefore cannot implement it.
///
/// The seal matters for `&mut T`. Rust treats `&mut T` as a type of the crate that defines `T`. A
/// crate that defines a component could otherwise implement `ReadOnlyQuery` for `&mut` of that
/// component, and `World::query` would then hand out mutable slices through `&World`.
///
/// # Safety
///
/// An implementation MUST guarantee that [`Query::find_columns`] builds only shared slices. A
/// mutable slice built through `&World` would let a caller change a component while another caller
/// reads it, which is undefined behaviour.
///
/// # Examples
///
/// The example accepts `&Position` as a `ReadOnlyQuery`.
///
/// ```
/// use enki::ReadOnlyQuery;
/// # use enki::Component;
///
/// struct Position(f32, f32);
/// # impl Component for Position {}
///
/// // Accepts any query that only reads
/// fn accept<Q: ReadOnlyQuery>() {}
///
/// // Passes a query that reads each `Position`
/// accept::<&Position>();
/// ```
///
/// The compiler rejects `&mut Position` in [`World::query`](crate::World::query), since
/// `find_columns` for `&mut T` builds a mutable slice.
///
/// ```compile_fail
/// use enki::World;
/// # use enki::Component;
///
/// struct Position(f32, f32);
/// # impl Component for Position {}
///
/// let world = World::new();
///
/// // Fails to compile, since `&mut Position` does not implement `ReadOnlyQuery`
/// for position in world.query::<&mut Position>() {}
/// ```
///
/// The compiler also rejects an implementation for `&mut Position` written outside the crate,
/// since `&mut Position` does not implement `ReadOnlySealed`.
///
/// ```compile_fail
/// use enki::ReadOnlyQuery;
/// # use enki::Component;
///
/// struct Position(f32, f32);
/// # impl Component for Position {}
///
/// // Fails to compile, since code outside `enki` cannot implement the sealing trait
/// unsafe impl ReadOnlyQuery for &mut Position {}
/// ```
pub unsafe trait ReadOnlyQuery: Query + sealed::ReadOnlySealed {}

impl<T: Component> sealed::ReadOnlySealed for &T {}

// SAFETY: `find_columns` for `&T` builds a `&[T]`, which is a shared slice
unsafe impl<T: Component> ReadOnlyQuery for &T {}

/// Implements [`Query`] for every tuple with one field per listed type parameter.
///
/// The call `impl_query!(A, B, C)` implements [`Query`] for `(A, B, C)`, where each of `A`, `B`
/// and `C` is a query, such as `&Position` or `&mut Velocity`. The file calls the macro once for
/// each size of tuple, from one field to thirty-two. The pattern, and the reuse of each type
/// parameter name as a variable name, work as in `impl_component_set!` in `set.rs`, whose
/// documentation explains both.
///
/// A field can itself be a tuple, since every tuple of queries is a query. `((&A, &B), &C)` is
/// therefore a query too, and its items arrive as `((a, b), c)`.
///
/// # Rows
///
/// `build_rows` turns the columns of each field into that field's rows. `count_rows` returns the
/// smallest row count among the fields. Every column in one table has one component per entity,
/// meaning the counts are equal in practice. `fetch_row` reads one row from every field, and puts
/// the results together into the item for that row.
macro_rules! impl_query {
    ($($name:ident),+) => {
        impl<$($name: Query),+> sealed::Sealed for ($($name,)+) {}

        // SAFETY: each field implements `Query`, and thereby meets both guarantees on its own.
        // `add_permissions` pushes every entry of each field, in field order. `find_columns`
        // borrows the columns that the fields borrow, from the one table that it receives
        unsafe impl<$($name: Query),+> Query for ($($name,)+) {
            type Columns<'w> = ($(<$name as Query>::Columns<'w>,)+);
            type Item<'w> = ($(<$name as Query>::Item<'w>,)+);

            fn add_permissions(permissions: &mut Vec<(TypeId, Permission)>) {
                $(<$name as Query>::add_permissions(permissions);)+
            }

            unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
                // SAFETY: `find_columns` requires the three guarantees in its Safety section. The
                // caller gives them for the tuple, and they carry over to each field.
                // `check_permissions` for the tuple covers every entry of every field. Two fields
                // can therefore share a column only where both read it, which builds two shared
                // slices
                Some(($(unsafe { <$name as Query>::find_columns(archetype) }?,)+))
            }

            type Rows<'w> = ($(<$name as Query>::Rows<'w>,)+);

            // Allows the variables below to take the type parameter names, which are in capitals
            #[expect(non_snake_case)]
            fn build_rows<'w>(columns: Self::Columns<'w>) -> Self::Rows<'w> {
                // Moves the columns of each field out of the tuple, into a variable with the
                // name of its type
                let ($($name,)+) = columns;
                ($(<$name as Query>::build_rows($name),)+)
            }

            #[expect(non_snake_case)]
            fn count_rows(rows: &Self::Rows<'_>) -> usize {
                let ($($name,)+) = rows;
                let mut count = usize::MAX;
                $(count = count.min(<$name as Query>::count_rows($name));)+
                count
            }

            #[expect(non_snake_case)]
            unsafe fn fetch_row<'w>(rows: &Self::Rows<'w>, row: usize) -> Self::Item<'w> {
                let ($($name,)+) = rows;
                // SAFETY: `fetch_row` requires a row below each field's count, read at most once.
                // `count_rows` for the tuple is the smallest count of any field, and the caller
                // keeps `row` below it. The caller reads each row of the tuple once, meaning it
                // reads each row of each field once
                ($(unsafe { <$name as Query>::fetch_row($name, row) },)+)
            }
        }

        impl<$($name: ReadOnlyQuery),+> sealed::ReadOnlySealed for ($($name,)+) {}

        // SAFETY: `find_columns` for a tuple builds the slices of its fields. Each field implements
        // `ReadOnlyQuery`, meaning each builds only shared slices
        unsafe impl<$($name: ReadOnlyQuery),+> ReadOnlyQuery for ($($name,)+) {}
    };
}

// Implements `Query` for every tuple of one to thirty-two queries
impl_query!(A);
impl_query!(A, B);
impl_query!(A, B, C);
impl_query!(A, B, C, D);
impl_query!(A, B, C, D, E);
impl_query!(A, B, C, D, E, F);
impl_query!(A, B, C, D, E, F, G);
impl_query!(A, B, C, D, E, F, G, H);
impl_query!(A, B, C, D, E, F, G, H, I);
impl_query!(A, B, C, D, E, F, G, H, I, J);
impl_query!(A, B, C, D, E, F, G, H, I, J, K);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
impl_query!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD,
    AE
);
impl_query!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD,
    AE, AF
);

/// Checks that the query type `Q` changes each component type only where it asks for that type
/// once, then returns the permissions that it checked.
///
/// A caller MUST run this check before it calls [`Query::find_columns`], as the Safety section of
/// that method sets out. The returned list contains the entries that [`Query::add_permissions`]
/// pushes for `Q`, in the same order. The world builds the query mask from that list, rather than
/// building a second one.
///
/// # Panics
///
/// Panics where `Q` asks for one component type twice, and changes it through at least one of the
/// two. `(&mut Position, &Position)` panics, for example, whereas `(&Position, &Position)` passes.
///
/// # Examples
///
/// `(&mut Position, &Velocity)` returns `[(Position, Write), (Velocity, Read)]`, each paired with
/// the `TypeId` of its type. The test `check_permissions_with_valid_queries_does_not_panic` runs
/// the check for nine queries that pass.
pub(crate) fn check_permissions<Q: Query>() -> Vec<(TypeId, Permission)> {
    let mut permissions = Vec::new();
    Q::add_permissions(&mut permissions);
    for (index, &(type_id, permission)) in permissions.iter().enumerate() {
        for &(other_id, other_permission) in &permissions[index + 1..] {
            // Records whether either permission of the pair changes its component
            let writes = permission == Permission::Write || other_permission == Permission::Write;
            // Panics where both permissions are for one type and at least one of them changes that
            // type
            if type_id == other_id && writes {
                panic!("a query must not change a component type that it also asks for elsewhere");
            }
        }
    }
    permissions
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;
    use crate::component::ComponentInfo;
    use crate::mask::ComponentMask;

    #[derive(Debug, PartialEq)]
    struct Position(f32);

    #[derive(Debug, PartialEq)]
    struct Velocity(f32);

    #[derive(Debug, PartialEq)]
    struct Name(String);

    /// Stands for a component type of size zero, which a column counts without storing any bytes.
    #[derive(Debug, PartialEq)]
    struct Tag;

    /// Stands for a component type that no table in these tests stores.
    #[derive(Debug, PartialEq)]
    struct Health(u32);

    impl Component for Position {}
    impl Component for Velocity {}
    impl Component for Name {}
    impl Component for Tag {}
    impl Component for Health {}

    /// Returns the entries that `Q` pushes onto an empty list.
    fn collect_permissions<Q: Query>() -> Vec<(TypeId, Permission)> {
        let mut permissions = Vec::new();
        Q::add_permissions(&mut permissions);
        permissions
    }

    /// Defines the query type with the most fields that `impl_query!` implements `Query` for. It
    /// reads `Position` in the first thirty-one fields, and changes `Velocity` in the last.
    type Query32 = (
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static Position,
        &'static mut Velocity,
    );

    /// Pushes `value` onto the column that stores `T`, which leaves the entity list unchanged.
    fn push_component<T: Component>(table: &mut Archetype, value: T) {
        let column = table
            .components_of_mut(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.push_typed(value);
    }

    /// Returns a table that stores `Position`, `Velocity`, `Name` and `Tag`, with one row for each
    /// entry in `values`.
    ///
    /// The row for the entry `x` stores `Position(x)`, `Velocity(x * 10.0)` and a `Name` of
    /// `"row x"`.
    fn build_table(values: &[f32]) -> Archetype {
        let mut table = Archetype::new(&[
            ComponentInfo::of::<Position>(ComponentMask::single(0)),
            ComponentInfo::of::<Velocity>(ComponentMask::single(1)),
            ComponentInfo::of::<Name>(ComponentMask::single(2)),
            ComponentInfo::of::<Tag>(ComponentMask::single(3)),
        ]);
        for (index, &x) in values.iter().enumerate() {
            table.push_entity(Entity::new(index as u32, 0), |table, _| {
                push_component(table, Position(x));
                push_component(table, Velocity(x * 10.0));
                push_component(table, Name(format!("row {x}")));
                push_component(table, Tag);
            });
        }
        table
    }

    /// Runs `check_permissions` for `Q`, then returns the columns in `table` that `Q` asks for.
    ///
    /// The columns borrow `table` exclusively, meaning the compiler rejects any other use of the
    /// table while they are in use.
    fn find_checked<Q: Query>(table: &mut Archetype) -> Option<Q::Columns<'_>> {
        check_permissions::<Q>();

        // SAFETY: `find_columns` requires three things. First, nothing outside the query reaches
        // the columns during the borrow, since `table` is an exclusive borrow that the returned
        // columns inherit. Second, `check_permissions` has returned for `Q`, on the line above.
        // Third, the method runs once for the table during the borrow, since the exclusive borrow
        // stops any second call until the columns are gone
        unsafe { Q::find_columns(table) }
    }

    /// Returns the column that stores `T`, as a slice of `T`.
    fn read_column<T: Component>(table: &Archetype) -> &[T] {
        let column = table
            .components_of(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.as_slice::<T>()
    }

    /// Checks the entries and their order for single types, pairs and pairs inside pairs
    #[test]
    fn add_permissions_with_each_query_shape_pushes_entries_in_order() {
        let position = TypeId::of::<Position>();
        let velocity = TypeId::of::<Velocity>();
        let tag = TypeId::of::<Tag>();
        let name = TypeId::of::<Name>();

        // Lists each query shape, the entries that it pushes, and the entries that it should push
        let cases = [
            (
                "&Position",
                collect_permissions::<&Position>(),
                vec![(position, Permission::Read)],
            ),
            (
                "&mut Position",
                collect_permissions::<&mut Position>(),
                vec![(position, Permission::Write)],
            ),
            (
                "(&mut Position, &Velocity)",
                collect_permissions::<(&mut Position, &Velocity)>(),
                vec![(position, Permission::Write), (velocity, Permission::Read)],
            ),
            (
                "(&Velocity, &mut Position)",
                collect_permissions::<(&Velocity, &mut Position)>(),
                vec![(velocity, Permission::Read), (position, Permission::Write)],
            ),
            (
                "((&Position, &mut Velocity), &Tag)",
                collect_permissions::<((&Position, &mut Velocity), &Tag)>(),
                vec![
                    (position, Permission::Read),
                    (velocity, Permission::Write),
                    (tag, Permission::Read),
                ],
            ),
            (
                "(&Position, (&Velocity, &mut Tag))",
                collect_permissions::<(&Position, (&Velocity, &mut Tag))>(),
                vec![
                    (position, Permission::Read),
                    (velocity, Permission::Read),
                    (tag, Permission::Write),
                ],
            ),
            (
                "(&Position, &Position)",
                collect_permissions::<(&Position, &Position)>(),
                vec![(position, Permission::Read), (position, Permission::Read)],
            ),
            (
                "(&mut Position,)",
                collect_permissions::<(&mut Position,)>(),
                vec![(position, Permission::Write)],
            ),
            (
                "(&Position, &mut Velocity, &Tag)",
                collect_permissions::<(&Position, &mut Velocity, &Tag)>(),
                vec![
                    (position, Permission::Read),
                    (velocity, Permission::Write),
                    (tag, Permission::Read),
                ],
            ),
            (
                "(&Tag, (&Position, &Velocity), &mut Name)",
                collect_permissions::<(&Tag, (&Position, &Velocity), &mut Name)>(),
                vec![
                    (tag, Permission::Read),
                    (position, Permission::Read),
                    (velocity, Permission::Read),
                    (name, Permission::Write),
                ],
            ),
        ];

        for (label, found, expected) in cases {
            assert_eq!(found, expected, "entries for {label}");
        }
    }

    /// Checks the largest tuple, of thirty-two fields, with one entry per field in field order
    #[test]
    fn add_permissions_with_32_fields_pushes_32_entries_in_order() {
        let position = TypeId::of::<Position>();
        let velocity = TypeId::of::<Velocity>();

        let found = collect_permissions::<Query32>();

        let mut expected = vec![(position, Permission::Read); 31];
        expected.push((velocity, Permission::Write));
        assert_eq!(found, expected);
    }

    /// Checks that the method adds its entries after the ones already in the list
    #[test]
    fn add_permissions_with_existing_entries_keeps_them_in_front() {
        let health = TypeId::of::<Health>();
        let position = TypeId::of::<Position>();
        let velocity = TypeId::of::<Velocity>();
        let mut permissions = vec![(health, Permission::Write)];

        <(&Position, &mut Velocity)>::add_permissions(&mut permissions);

        assert_eq!(
            permissions,
            [
                (health, Permission::Write),
                (position, Permission::Read),
                (velocity, Permission::Write),
            ]
        );
    }

    /// Checks that every query asking for each changed type once passes, including repeated reads
    #[test]
    fn check_permissions_with_valid_queries_does_not_panic() {
        // Lists the check for each query that should pass
        let cases: [fn() -> _; 9] = [
            check_permissions::<&Position>,
            check_permissions::<&mut Position>,
            check_permissions::<(&Position, &Velocity)>,
            check_permissions::<(&mut Position, &mut Velocity)>,
            check_permissions::<(&Position, &Position)>,
            check_permissions::<((&Position, &Velocity), &Position)>,
            check_permissions::<((&mut Position, &Velocity), &mut Tag)>,
            check_permissions::<(&mut Tag, (&Position, &mut Name))>,
            check_permissions::<(&mut Position, &Velocity, &mut Tag)>,
        ];

        for check in cases {
            check();
        }
    }

    /// Checks that a query that changes a type and then reads it panics
    #[test]
    #[should_panic]
    fn check_permissions_with_write_then_read_panics() {
        check_permissions::<(&mut Position, &Position)>();
    }

    /// Checks that a query that reads a type and then changes it panics
    #[test]
    #[should_panic]
    fn check_permissions_with_read_then_write_panics() {
        check_permissions::<(&Position, &mut Position)>();
    }

    /// Checks that a query that changes one type twice panics
    #[test]
    #[should_panic]
    fn check_permissions_with_write_then_write_panics() {
        check_permissions::<(&mut Position, &mut Position)>();
    }

    /// Checks that a changed type inside a pair still clashes with the same type outside it
    #[test]
    #[should_panic]
    fn check_permissions_with_duplicate_in_nested_first_part_panics() {
        check_permissions::<((&mut Position, &Velocity), &Position)>();
    }

    /// Checks that a changed type outside a pair still clashes with the same type inside it
    #[test]
    #[should_panic]
    fn check_permissions_with_duplicate_in_nested_second_part_panics() {
        check_permissions::<(&mut Position, (&Velocity, &Position))>();
    }

    /// Checks that a clash between two separate inner pairs panics
    #[test]
    #[should_panic]
    fn check_permissions_with_duplicate_across_nested_pairs_panics() {
        check_permissions::<((&Velocity, &Position), (&Tag, &mut Position))>();
    }

    /// Checks that a changed type in the first field of a flat tuple clashes with the same type in
    /// the last field
    #[test]
    #[should_panic]
    fn check_permissions_with_duplicate_across_flat_tuple_panics() {
        check_permissions::<(&mut Position, &Velocity, &Tag, &Position)>();
    }

    /// Checks that a size-zero type that the query changes twice panics like any other type
    #[test]
    #[should_panic]
    fn check_permissions_with_duplicate_zero_size_type_panics() {
        check_permissions::<(&mut Tag, &mut Tag)>();
    }

    /// Checks that a missing type gives `None`, alone and on either side of a pair
    #[test]
    fn find_columns_with_missing_type_returns_none() {
        let mut table = build_table(&[1.0, 2.0]);

        // Lists each query, with whether `find_columns` returned `None` for it
        let cases = [
            ("&Health", find_checked::<&Health>(&mut table).is_none()),
            (
                "&mut Health",
                find_checked::<&mut Health>(&mut table).is_none(),
            ),
            (
                "(&Health, &Position)",
                find_checked::<(&Health, &Position)>(&mut table).is_none(),
            ),
            (
                "(&Position, &Health)",
                find_checked::<(&Position, &Health)>(&mut table).is_none(),
            ),
            (
                "(&mut Health, &mut Position)",
                find_checked::<(&mut Health, &mut Position)>(&mut table).is_none(),
            ),
            (
                "(&mut Position, &mut Health)",
                find_checked::<(&mut Position, &mut Health)>(&mut table).is_none(),
            ),
            (
                "((&Position, &Velocity), &mut Health)",
                find_checked::<((&Position, &Velocity), &mut Health)>(&mut table).is_none(),
            ),
            (
                "(&Position, &mut Velocity, &Health)",
                find_checked::<(&Position, &mut Velocity, &Health)>(&mut table).is_none(),
            ),
        ];

        for (label, is_none) in cases {
            assert!(is_none, "result for {label}");
        }
    }

    /// Checks that a table with no rows still gives every column, each one empty
    #[test]
    fn find_columns_with_empty_table_returns_empty_columns() {
        let mut table = build_table(&[]);

        let ((positions, velocities), tags) =
            find_checked::<((&Position, &mut Velocity), &mut Tag)>(&mut table)
                .expect("the table should store every type in the query");

        assert!(positions.is_empty(), "positions");
        assert!(velocities.is_empty(), "velocities");
        assert!(tags.is_empty(), "tags");
    }

    /// Checks that a read returns the whole column, in row order
    #[test]
    fn find_columns_with_read_query_returns_column_in_row_order() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);

        let positions =
            find_checked::<&Position>(&mut table).expect("the table should store `Position`");

        assert_eq!(positions, [Position(1.0), Position(2.0), Position(3.0)]);
    }

    /// Checks that a query that reads one type twice gets the same column on both sides
    #[test]
    fn find_columns_with_duplicate_reads_returns_same_column_twice() {
        let mut table = build_table(&[1.0, 2.0]);

        let (first, second) = find_checked::<(&Position, &Position)>(&mut table)
            .expect("the table should store `Position`");

        assert_eq!(first, [Position(1.0), Position(2.0)], "first column");
        assert_eq!(second, [Position(1.0), Position(2.0)], "second column");
    }

    /// Checks that writes through the mutable column reach the table, after the column moved to a
    /// larger block
    #[test]
    fn find_columns_with_write_query_changes_reach_table() {
        // Pushes 5 rows, which makes the `Position` column grow from 4 to 8
        let mut table = build_table(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        let positions =
            find_checked::<&mut Position>(&mut table).expect("the table should store `Position`");
        for position in positions.iter_mut() {
            position.0 *= 2.0;
        }

        assert_eq!(
            read_column::<Position>(&table),
            [
                Position(2.0),
                Position(4.0),
                Position(6.0),
                Position(8.0),
                Position(10.0),
            ]
        );
    }

    /// Checks that two mutable columns at once each reach their own column in the table
    #[test]
    fn find_columns_with_two_write_columns_changes_both() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);

        let (positions, velocities) = find_checked::<(&mut Position, &mut Velocity)>(&mut table)
            .expect("the table should store `Position` and `Velocity`");
        for (position, velocity) in positions.iter_mut().zip(velocities.iter_mut()) {
            position.0 += velocity.0;
            velocity.0 = 0.0;
        }

        assert_eq!(
            read_column::<Position>(&table),
            [Position(11.0), Position(22.0), Position(33.0)],
            "positions"
        );
        assert_eq!(
            read_column::<Velocity>(&table),
            [Velocity(0.0), Velocity(0.0), Velocity(0.0)],
            "velocities"
        );
    }

    /// Checks that replacing a component with heap data through a mutable column drops the old one
    /// and keeps the new one in the table
    #[test]
    fn find_columns_with_write_query_replaces_heap_components() {
        let mut table = build_table(&[1.0, 2.0]);

        let names = find_checked::<&mut Name>(&mut table).expect("the table should store `Name`");
        names[0] = Name(String::from("Ada"));
        names[1].0.push_str(" and more");

        assert_eq!(
            read_column::<Name>(&table),
            [
                Name(String::from("Ada")),
                Name(String::from("row 2 and more"))
            ]
        );
    }

    /// Checks that a mutable column of a size-zero type has one entry for each row, and takes
    /// writes
    #[test]
    fn find_columns_with_zero_size_write_query_returns_column_of_row_count() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);

        let tags = find_checked::<&mut Tag>(&mut table).expect("the table should store `Tag`");
        for tag in tags.iter_mut() {
            *tag = Tag;
        }
        let found = tags.len();

        assert_eq!(found, 3, "length of the column");
        assert_eq!(read_column::<Tag>(&table).len(), 3, "length in the table");
    }

    /// Checks that a pair inside a pair returns each column with its contents
    #[test]
    fn find_columns_with_nested_pair_returns_each_column() {
        let mut table = build_table(&[1.0, 2.0]);

        let (positions, (velocities, tags)) =
            find_checked::<(&Position, (&mut Velocity, &Tag))>(&mut table)
                .expect("the table should store every type in the query");
        velocities[1].0 = -1.0;
        let found_velocities: Vec<f32> = velocities.iter().map(|velocity| velocity.0).collect();
        let tag_count = tags.len();

        assert_eq!(positions, [Position(1.0), Position(2.0)], "positions");
        assert_eq!(found_velocities, [10.0, -1.0], "velocities");
        assert_eq!(tag_count, 2, "tags");
        assert_eq!(
            read_column::<Velocity>(&table),
            [Velocity(10.0), Velocity(-1.0)],
            "velocities in the table"
        );
    }

    /// Checks that a single type yields one item for each row, in row order
    #[test]
    fn split_rows_with_read_query_yields_items_in_row_order() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);
        let positions =
            find_checked::<&Position>(&mut table).expect("the table should store `Position`");

        let items: Vec<&Position> = <&Position>::split_rows(positions).collect();

        assert_eq!(items, [&Position(1.0), &Position(2.0), &Position(3.0)]);
    }

    /// Checks that a pair yields the components of one row together
    #[test]
    fn split_rows_with_pair_zips_items_by_row() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);
        let columns = find_checked::<(&Position, &Velocity)>(&mut table)
            .expect("the table should store `Position` and `Velocity`");

        let items: Vec<(&Position, &Velocity)> =
            <(&Position, &Velocity)>::split_rows(columns).collect();

        assert_eq!(
            items,
            [
                (&Position(1.0), &Velocity(10.0)),
                (&Position(2.0), &Velocity(20.0)),
                (&Position(3.0), &Velocity(30.0)),
            ]
        );
    }

    /// Checks that changes through the items of a mixed query reach the table
    #[test]
    fn split_rows_with_write_query_changes_reach_table() {
        let mut table = build_table(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let columns = find_checked::<(&mut Position, &Velocity)>(&mut table)
            .expect("the table should store `Position` and `Velocity`");

        for (position, velocity) in <(&mut Position, &Velocity)>::split_rows(columns) {
            position.0 += velocity.0;
        }

        assert_eq!(
            read_column::<Position>(&table),
            [
                Position(11.0),
                Position(22.0),
                Position(33.0),
                Position(44.0),
                Position(55.0),
            ]
        );
    }

    /// Checks that a pair inside a pair yields every component of one row, in row order
    #[test]
    fn split_rows_with_nested_pair_yields_items_by_row() {
        let mut table = build_table(&[1.0, 2.0]);
        let columns = find_checked::<((&mut Velocity, &Name), &Position)>(&mut table)
            .expect("the table should store every type in the query");

        let mut items = Vec::new();
        for ((velocity, name), position) in
            <((&mut Velocity, &Name), &Position)>::split_rows(columns)
        {
            velocity.0 = position.0;
            items.push((name.0.clone(), position.0));
        }

        assert_eq!(
            items,
            [(String::from("row 1"), 1.0), (String::from("row 2"), 2.0)],
            "items"
        );
        assert_eq!(
            read_column::<Velocity>(&table),
            [Velocity(1.0), Velocity(2.0)],
            "velocities in the table"
        );
    }

    /// Checks that a flat tuple yields every component of one row together, in row order, and that
    /// a change through one field reaches the table
    #[test]
    fn split_rows_with_flat_tuple_yields_items_by_row() {
        let mut table = build_table(&[1.0, 2.0]);
        let columns = find_checked::<(&mut Velocity, &Name, &Position)>(&mut table)
            .expect("the table should store every type in the query");

        let mut items = Vec::new();
        for (velocity, name, position) in <(&mut Velocity, &Name, &Position)>::split_rows(columns) {
            velocity.0 = position.0;
            items.push((name.0.clone(), position.0));
        }

        assert_eq!(
            items,
            [(String::from("row 1"), 1.0), (String::from("row 2"), 2.0)],
            "items"
        );
        assert_eq!(
            read_column::<Velocity>(&table),
            [Velocity(1.0), Velocity(2.0)],
            "velocities in the table"
        );
    }

    /// Checks that a table with no rows yields no items, for a read, a write and a pair
    #[test]
    fn split_rows_with_empty_table_yields_no_items() {
        let mut table = build_table(&[]);

        // Lists each query, with the number of items that it yields
        let cases = [
            (
                "&Position",
                find_checked::<&Position>(&mut table)
                    .map(|columns| <&Position>::split_rows(columns).count()),
            ),
            (
                "&mut Position",
                find_checked::<&mut Position>(&mut table)
                    .map(|columns| <&mut Position>::split_rows(columns).count()),
            ),
            (
                "(&mut Position, &Tag)",
                find_checked::<(&mut Position, &Tag)>(&mut table)
                    .map(|columns| <(&mut Position, &Tag)>::split_rows(columns).count()),
            ),
        ];

        for (label, count) in cases {
            assert_eq!(count, Some(0), "items for {label}");
        }
    }

    /// Checks that a size-zero type still yields one item for each row, beside a type with data
    #[test]
    fn split_rows_with_zero_size_type_yields_one_item_per_row() {
        let mut table = build_table(&[1.0, 2.0, 3.0]);
        let columns = find_checked::<(&mut Tag, &Position)>(&mut table)
            .expect("the table should store `Tag` and `Position`");

        let mut positions = Vec::new();
        for (tag, position) in <(&mut Tag, &Position)>::split_rows(columns) {
            *tag = Tag;
            positions.push(position.0);
        }

        assert_eq!(positions, [1.0, 2.0, 3.0]);
    }
}
