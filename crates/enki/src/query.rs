use std::any::TypeId;
use std::{iter, slice};
use crate::archetype::Archetype;
use crate::Component;

// Whether a query reads a component type or changes it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,  // the query reads the component
    Write, // the query mutates the component
}

pub unsafe trait Query {
    type Columns<'w>;

    type Item<'w>;

    fn add_access(accesses: &mut Vec<(TypeId, Access)>);

    unsafe fn find_columns<'w>(archetype: &'w Archetype) -> Option<Self::Columns<'w>>;

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

pub(crate) fn check_access<Q: Query>() {
    let mut accesses = Vec::new();
    Q::add_access(&mut accesses);
    for (index, &(type_id, access)) in accesses.iter().enumerate() {
        for &(other_id, other_access) in &accesses[index + 1..] {
            // Records whether either access of the pair changes its component
            let writes = access == Access::Write || other_access == Access::Write;
            // Panics where both accesses are for one type and at least one of them changes it
            if type_id == other_id && writes {
                panic!("a query must not change a component type that it also asks for elsewhere");
            }
        }
    }
}