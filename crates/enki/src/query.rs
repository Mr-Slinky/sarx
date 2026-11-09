//! Defines [`Query`], the trait that describes which component types a caller visits on each
//! entity. A query also states whether the caller reads or changes each of those types.
//!
//! A caller writes a query as a type. `&T` reads the `T` of each entity, and `&mut T` changes it. A
//! pair such as `(&mut Position, &Velocity)` asks for both types at once. A pair can contain
//! another pair, which lets a query ask for three or more types.
//!
//! The caller passes the query type to `World::query_mut`, which returns one item for each entity
//! that has every type in the query. The documentation on `World::query_mut` shows a full example.
//!
//! # Rules on access
//!
//! A query changes a component type only where it asks for that type once. A query such as
//! `(&mut Position, &Position)` would hand out two references to the same `Position`, one of them
//! able to change it. Rust forbids that pair of references. [`check_access`] therefore panics on
//! such a query, before the world hands out any reference.

use crate::Component;
use crate::archetype::Archetype;
use std::any::TypeId;
use std::{iter, slice};

/// Represents whether a query reads a component type or changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The query reads the component, through a `&T`.
    Read,
    /// The query changes the component, through a `&mut T`.
    Write,
}

/// Describes the component types that a query asks for, and how the world hands them out.
///
/// A query type is `&T` for a component that the caller reads, or `&mut T` for one that it changes.
/// A pair `(A, B)` of two query types is also a query, and asks for every type that `A` and `B` ask
/// for.
///
/// The world runs a query in three steps. It first calls [`Query::add_access`] to collect the
/// component types that the query asks for. It then calls [`Query::find_columns`] to borrow the
/// matching columns of each table. Finally, it calls [`Query::split_rows`] to split those columns
/// into one item per entity. `World::query_tables_mut` stops after the second step, and returns
/// the columns.
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
/// crate implements it for. Code outside the crate cannot implement the trait. An implementation
/// has to write `Access` and `Archetype` in its method signatures, and code outside the crate
/// cannot write a path to either type.
///
/// # Safety
///
/// An implementation MUST guarantee the following:
///
/// - [`Query::add_access`] pushes one entry for every component type that
///   [`Query::find_columns`] borrows. The entry is `Access::Write` for every type that
///   `find_columns` borrows mutably.
/// - [`Query::find_columns`] borrows only the columns of the table that it receives.
///
/// `check_access` relies on the first guarantee to reject a query that borrows one column mutably
/// and also borrows it a second time. A missing entry would let a mutable reference reach a
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
///
/// struct Position(f32, f32);
/// struct Velocity(f32, f32);
///
/// impl enki::Component for Position {}
/// impl enki::Component for Velocity {}
///
/// // Accepts any query type
/// fn accept<Q: Query>() {}
///
/// // Passes a query that changes each `Position` and reads each `Velocity`
/// accept::<(&mut Position, &Velocity)>();
/// ```
pub unsafe trait Query {
    /// Represents the columns that the query borrows from one table, for the lifetime `'w`.
    ///
    /// `&T` borrows its column as a `&'w [T]`, and `&mut T` as a `&'w mut [T]`. A pair borrows the
    /// columns that each of its parts borrows.
    type Columns<'w>;

    /// Represents the item that the query produces for one entity, for the lifetime `'w`.
    ///
    /// `&T` produces a `&'w T`, and `&mut T` a `&'w mut T`. A pair produces a pair that contains
    /// the item that each part produces.
    type Item<'w>;

    /// Pushes one entry onto `accesses` for each component type that the query asks for.
    ///
    /// Each entry pairs the [`TypeId`] of the type with the `Access` that the query needs. A pair
    /// pushes the entries of its first part, then those of its second part. The method leaves every
    /// existing entry in `accesses` in place.
    fn add_access(accesses: &mut Vec<(TypeId, Access)>);

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
    /// - Nothing outside the query reads or changes the columns of `archetype` during `'w`. A
    ///   caller that borrows the world through `&mut World` for `'w` meets this guarantee.
    /// - `check_access` has already returned for the query type, without a panic.
    /// - The caller calls the method at most once for each table during `'w`.
    ///
    /// A broken guarantee lets a mutable slice reach a component that another reference also
    /// reaches, which is undefined behaviour.
    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>>;

    /// Returns an iterator over the rows in `columns`, with one item for each entity in the table.
    ///
    /// The items come in row order. The item for each entity borrows from the columns for the
    /// lifetime `'w`.
    fn split_rows<'w>(columns: Self::Columns<'w>) -> impl Iterator<Item = Self::Item<'w>>;
}

unsafe impl<T: Component> Query for &T {
    type Columns<'w> = &'w [T];
    type Item<'w> = &'w T;

    fn add_access(accesses: &mut Vec<(TypeId, Access)>) {
        accesses.push((TypeId::of::<T>(), Access::Read));
    }

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
        let components = archetype.components_of(TypeId::of::<T>())?;
        Some(components.as_slice::<T>())
    }

    fn split_rows<'w>(columns: Self::Columns<'w>) -> impl Iterator<Item = Self::Item<'w>> {
        columns.iter()
    }
}

unsafe impl<T: Component> Query for &mut T {
    type Columns<'w> = &'w mut [T];
    type Item<'w> = &'w mut T;

    fn add_access(accesses: &mut Vec<(TypeId, Access)>) {
        accesses.push((TypeId::of::<T>(), Access::Write));
    }

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
        let components = archetype.components_of(TypeId::of::<T>())?;
        let start = components.data().cast::<T>();
        let len = components.len();
        Some(unsafe { slice::from_raw_parts_mut(start.as_ptr(), len) })
    }

    fn split_rows<'w>(columns: Self::Columns<'w>) -> impl Iterator<Item = Self::Item<'w>> {
        columns.iter_mut()
    }
}

unsafe impl<A: Query, B: Query> Query for (A, B) {
    type Columns<'w> = (A::Columns<'w>, B::Columns<'w>);
    type Item<'w> = (A::Item<'w>, B::Item<'w>);

    fn add_access(accesses: &mut Vec<(TypeId, Access)>) {
        A::add_access(accesses);
        B::add_access(accesses);
    }

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>> {
        let a = unsafe { A::find_columns(archetype) }?;
        let b = unsafe { B::find_columns(archetype) }?;
        Some((a, b))
    }

    fn split_rows<'w>((a, b): Self::Columns<'w>) -> impl Iterator<Item = Self::Item<'w>> {
        let a_rows = A::split_rows(a);
        let b_rows = B::split_rows(b);
        iter::zip(a_rows, b_rows)
    }
}

/// Checks that the query type `Q` changes each component type only where it asks for that type
/// once.
///
/// A caller MUST run this check before it calls [`Query::find_columns`], as the Safety section of
/// that method sets out.
///
/// # Panics
///
/// Panics where `Q` asks for one component type twice, and changes it through at least one of the
/// two. `(&mut Position, &Position)` panics, for example, whereas `(&Position, &Position)` passes.
pub(crate) fn check_access<Q: Query>() {
    let mut accesses = Vec::new();
    Q::add_access(&mut accesses);
    for (index, &(type_id, access)) in accesses.iter().enumerate() {
        for &(other_id, other_access) in &accesses[index + 1..] {
            // Records whether either access of the pair changes its component
            let writes = access == Access::Write || other_access == Access::Write;
            // Panics where both accesses are for one type and at least one of them changes that type
            if type_id == other_id && writes {
                panic!("a query must not change a component type that it also asks for elsewhere");
            }
        }
    }
}
