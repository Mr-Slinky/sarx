//! Defines [`Bundle`], the trait that every tuple of one to thirty-two components implements.
//!
//! # Usage
//!
//! A caller gives an entity several components in one call by passing them to
//! `World::add_components` as a tuple. That tuple is a bundle. Each field is one component, and the
//! fields can come in any order.
//!
//! The example below spawns an entity, then gives it three components in one call.
//!
//! ```ignore
//! let mut world = World::new();
//! let player = world.spawn_empty_ent();
//!
//! // Gives the player a `Position`, a `Velocity` and a `Health`
//! world.add_components(player, (Position(0.0, 0.0), Velocity(1.0, 0.0), Health(100)));
//! ```
//!
//! A bundle of one component takes a comma after the component, as in `(Health(100),)`. Rust parses
//! `(Health(100))` as a `Health` in brackets, rather than as a tuple.
//!
//! A tuple of thirty-three or more components does not implement [`Bundle`]. The compiler rejects
//! such a tuple as the argument to `add_components`.

use crate::archetype::Archetype;
use crate::component::{Component, ComponentInfo};
use crate::mask::ComponentMask;
use crate::world::World;
use std::any::TypeId;
use std::mem;

// ========================================================================================== \\
//                                          Modules                                           \\
// ========================================================================================== \\

/// Declares the trait that seals [`Bundle`].
mod sealed {
    /// Marks each type that the crate implements [`Bundle`](super::Bundle) for.
    ///
    /// The trait is `pub` inside a private module. Code outside the crate therefore cannot write a
    /// path to it, and cannot implement it.
    pub trait Sealed {}
}

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Represents a group of components, written as a tuple with one field per component.
///
/// Each field of a bundle must implement [`Component`].
///
/// The module documentation in `bundle.rs` shows how a caller passes a bundle to `World`.
///
/// # Visibility
///
/// Code outside the crate can use the trait as a bound, and can write tuples that implement it. The
/// attribute `#[doc(hidden)]` hides the two methods from the public documentation, since only code
/// inside the crate needs them.
///
/// Code outside the crate cannot implement the trait. `Bundle` requires the trait `Sealed`, which a
/// private module in `bundle.rs` declares. Code outside the crate cannot write a path to `Sealed`,
/// and therefore cannot implement it. As a result, the tuple implementations in the crate are the
/// only ones.
///
/// A method on `World` that adds a bundle must rely on that guarantee. The method must build a
/// table from the `ComponentInfo` values that [`Bundle::infos`] returns. It must then rely on
/// [`Bundle::push_into`] to push one component onto each new column.
///
/// # Examples
///
/// The example creates a tuple of a `Position` and a `Health`, and passes it to a function that
/// accepts any bundle.
///
/// ```
/// use enki::{Bundle, Component};
///
/// struct Position(f32, f32);
/// struct Health(u32);
///
/// impl Component for Position {}
/// impl Component for Health {}
///
/// // Accepts any bundle
/// fn accept<B: Bundle>(_bundle: B) {}
///
/// // Passes a tuple of a `Position` and a `Health`. The tuple implements `Bundle`
/// accept((Position(0.0, 0.0), Health(100)));
/// ```
pub trait Bundle: sealed::Sealed + Send + Sync + 'static {
    /// Returns one [`ComponentInfo`] for each type in the bundle, in the order of the tuple fields.
    ///
    /// The method has no `self` parameter. A caller therefore calls it on the type, as in
    /// `B::infos(world)`. The result depends on the types in the bundle, and on the bits that
    /// `world` assigns to them.
    ///
    /// # Arguments
    ///
    /// * `world`: the world that assigns each type its bit. Where a type has no bit in the world
    ///   yet, the world gives it the next free bit.
    ///
    /// # Panics
    ///
    /// Panics where one of the types has no bit in the world yet, and the world already has
    /// `ComponentMask::CAPACITY` component types.
    ///
    /// # Examples
    ///
    /// <b>From inside a generic method on `World`</b>
    ///
    /// The example calls `infos` inside a method such as `add_components`. In that method, `B` is the
    /// type of the bundle, and `self` is the `&mut World`. The call `B::infos(self)` borrows the
    /// world through `self` for the length of the call only. The method can then use `self` again
    /// once the call returns.
    ///
    /// ```ignore
    /// pub fn add_components<B: Bundle>(&mut self, entity: Entity, components: B) {
    ///     // Returns one `ComponentInfo` for each type in `B`, and gives each type a bit where the
    ///     // world has no bit for it yet
    ///     let infos: Vec<ComponentInfo> = B::infos(self);
    ///
    ///     // ...
    /// }
    /// ```
    ///
    /// <b>For a tuple type written out in full</b>
    ///
    /// The example calls `infos` for the tuple type `(Position, Health)`. The call writes out the
    /// tuple type and the trait in full. Written that way, the call shows that the implementation
    /// for tuples of two fields runs.
    ///
    /// ```ignore
    /// // Returns two `ComponentInfo` values, the first for `Position` and the second for `Health`
    /// let infos: Vec<ComponentInfo> = <(Position, Health) as Bundle>::infos(&mut world);
    /// ```
    #[doc(hidden)]
    fn infos(world: &mut World) -> Vec<ComponentInfo>;

    /// Stores each component in the bundle in `table`, at row `row`.
    ///
    /// Each component goes into the column in `table` that stores its type. Where `existing` has
    /// the bit for that type, the entity already had a component of that type, and the new component
    /// replaces the one at `row`. The method drops the old one. Otherwise the method pushes the new
    /// component onto the end of the column.
    ///
    /// The method drops the replaced components only once every component is stored. A `Drop`
    /// implementation that panics therefore leaves every column at its new length.
    ///
    /// The method takes the bundle by value, meaning the table owns each component from then on.
    ///
    /// # Arguments
    ///
    /// * `table`: the table that receives the components. It MUST have a column for each type in
    ///   the bundle.
    /// * `row`: the entity's row in `table`. For a push, the column MUST contain `row` components,
    ///   so that the push lands at `row`.
    /// * `existing`: the mask of the table that the entity was in before this call. It has a bit
    ///   for each type that the entity already had.
    ///
    /// # Panics
    ///
    /// Panics where `table` has no column for one of the types in the bundle, or where a column for
    /// a type in `existing` has no component at `row`. Also panics where the `Drop` implementation
    /// of a replaced component panics.
    ///
    /// # Examples
    ///
    /// The method has a `self` parameter, unlike [`Bundle::infos`]. A caller therefore calls it on
    /// the bundle itself, with the dot syntax.
    ///
    /// The example stores a bundle of a `Position` and a `Health` in `target`, a table with a
    /// `Position` column and a `Health` column. The entity has already moved into `target`, at row
    /// `row`. The entity had a `Health` before the move, and lacked a `Position`. `existing` is the
    /// mask of the entity's old table, which has the bit for `Health` and lacks the bit for
    /// `Position`.
    ///
    /// ```ignore
    /// // Creates a tuple of a `Position` and a `Health`. The tuple implements `Bundle`
    /// let components: (Position, Health) = (Position(0.0, 0.0), Health(100));
    ///
    /// // Borrows the table that the entity moved into
    /// let target: &mut Archetype = &mut self.archetypes[target_index];
    ///
    /// // Pushes the `Position` onto the `Position` column, and replaces the entity's `Health`
    /// components.push_into(target, row, existing);
    ///
    /// // `push_into` took `components` by value. The compiler therefore rejects any use of
    /// // `components` from here on
    /// ```
    #[doc(hidden)]
    fn push_into(self, table: &mut Archetype, row: usize, existing: ComponentMask);
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Stores `value` at row `row` of the column in `table` that stores `T`.
///
/// Where `existing` has the bit for `T`, the entity already had a `T`. `value` then replaces the
/// component at `row`, and the function returns the replaced component. Otherwise the function
/// pushes `value` onto the end of the column, and returns `None`.
///
/// The caller owns the returned component, and decides when it drops.
///
/// # Panics
///
/// Panics where `table` has no column for `T`, or where `existing` has the bit for `T` and the
/// column has no component at `row`.
fn push_or_replace<T: Component>(
    table: &mut Archetype,
    row: usize,
    value: T,
    existing: ComponentMask,
) -> Option<T> {
    // Finds the column for `T`, and panics where the table has none
    let column = table
        .components_of_mut(TypeId::of::<T>())
        .expect("the table should have a column for every type in the bundle");

    // Replaces the component at `row`, where the entity already had a `T`
    let bit = column.info().bit();
    if existing.intersects(bit) {
        // Finds the component at `row`
        let slot = &mut column.as_mut_slice::<T>()[row];
        // Puts `value` in its place, and returns the old component for the caller to drop
        return Some(mem::replace(slot, value));
    }

    // Moves `value` onto the end of the column
    column.push_typed(value);
    // Reports that no component was replaced
    None
}

/// Implements [`Bundle`] for every tuple with one field per listed type parameter.
///
/// The call `impl_bundle!(A, B, C)` implements [`Bundle`] for `(A, B, C)`, where each of `A`, `B`
/// and `C` is any type that implements [`Component`]. The macro has no `#[macro_export]`,
/// meaning code outside the crate cannot call it.
///
/// # Pattern
///
/// The pattern `$($name:ident),+` matches one or more names, separated by commas. In the
/// expansion, each group written `$( ... )` repeats once for each matched name, with `$name`
/// standing for that name.
///
/// # Calls
///
/// The file calls the macro thirty-two times, once for each size of tuple from one field to
/// thirty-two. The type parameter names run from `A` to `Z`, then from `AA` to `AF`.
///
/// ```ignore
/// impl_bundle!(A);        // Implements `Bundle` for every tuple of one field
/// impl_bundle!(A, B);     // Implements `Bundle` for every tuple of two fields
/// impl_bundle!(A, B, C);  // Implements `Bundle` for every tuple of three fields
/// ...
/// impl_bundle!(
///     A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC,
///     AD, AE, AF
/// );
/// ```
///
/// The number of fields in a tuple decides which call implements [`Bundle`] for it. The tuple
/// `(Position, Velocity, Health)` has three fields. Its implementation therefore comes from
/// `impl_bundle!(A, B, C)`. In that implementation, `A` stands for `Position`, `B` for `Velocity`
/// and `C` for `Health`.
///
/// The tuple `(Health, Position, Velocity)` also has three fields, and uses the implementation from
/// the same call. In that implementation, `A` stands for `Health`, `B` for `Position` and `C` for
/// `Velocity`.
///
/// # Expansion of one call
///
/// The call `impl_bundle!(A, B, C)` expands to the code below, in full. This documentation adds the
/// comments, to explain each line. The expansion itself has no comments.
///
/// ```ignore
/// // Marks every tuple of three components as a type that the crate implements `Bundle` for,
/// // which `Bundle` requires
/// impl<A: Component, B: Component, C: Component> sealed::Sealed for (A, B, C,) {}
///
/// // Implements `Bundle` for every tuple of three types, where each type implements `Component`
/// impl<A: Component, B: Component, C: Component> Bundle for (A, B, C,) {
///     // Returns one `ComponentInfo` per type, in the order of the tuple fields
///     fn infos(world: &mut World) -> Vec<ComponentInfo> {
///         vec![
///             {
///                 // Finds the bit for `A`, and assigns the next free bit where the world has no
///                 // bit for `A` yet
///                 let bit = world.find_or_assign_bit(TypeId::of::<A>());
///                 // Builds the `ComponentInfo` for `A`, with that bit
///                 ComponentInfo::of::<A>(bit)
///             },
///             {
///                 // Does the same for `B`
///                 let bit = world.find_or_assign_bit(TypeId::of::<B>());
///                 ComponentInfo::of::<B>(bit)
///             },
///             {
///                 // Does the same for `C`
///                 let bit = world.find_or_assign_bit(TypeId::of::<C>());
///                 ComponentInfo::of::<C>(bit)
///             }
///         ]
///     }
///
///     // Allows the variables `A`, `B` and `C` below to have names in capitals
///     #[allow(non_snake_case)]
///     fn push_into(self, table: &mut Archetype, row: usize, existing: ComponentMask) {
///         // Moves the three fields out of the tuple, into the variables `A`, `B` and `C`
///         let (A, B, C,) = self;
///
///         // Stores each value at `row`, and keeps each replaced value in the tuple `replaced`
///         let replaced = (
///             push_or_replace(table, row, A, existing),
///             push_or_replace(table, row, B, existing),
///             push_or_replace(table, row, C, existing),
///         );
///
///         // Drops the replaced values, now that every column has its new length
///         drop(replaced);
///     }
/// }
/// ```
///
/// The compiler works out the type parameter of each `push_or_replace` call from the value that
/// the call passes. The call `push_or_replace(table, row, A, existing)` therefore runs with `T` as
/// the type `A`.
///
/// In `push_into`, each type parameter name is also the name of a variable. The line
/// `let (A, B, C,) = self;` creates the variables `A`, `B` and `C`. The variable `A` contains the
/// field of type `A`. Rust looks up type names and variable names in separate namespaces. As a
/// result, `TypeId::of::<A>()` refers to the type, while `push_or_replace(table, row, A, existing)`
/// refers to the variable.
///
/// The comma after the last name in `(A, B, C,)` changes the meaning only in the call with one
/// name. Rust parses `(A)` as the type `A` in brackets, while `(A,)` is a tuple with one field.
///
/// # Calls past thirty-two
///
/// A tuple of thirty-three components needs a new call, below the call for thirty-two. The new
/// call lists the same names as the call for thirty-two, followed by one more name, `AG`.
macro_rules! impl_bundle {
    ($($name:ident),+) => {
        // Marks the tuple as a type that the crate implements `Bundle` for, which `Bundle` requires
        impl<$($name: Component),+> sealed::Sealed for ($($name,)+) {}

        // Declares one type parameter per name, each with the component bound, and implements
        // `Bundle` for the tuple of those parameters. The comma after each name turns a single
        // name into a tuple of one field
        impl<$($name: Component),+> Bundle for ($($name,)+) {
            fn infos(world: &mut World) -> Vec<ComponentInfo> {
                // Builds one `ComponentInfo` per type, in the order of the tuple fields
                vec![$({
                    // Finds the bit for the type, and assigns the next free bit where the world
                    // has no bit for the type yet
                    let bit = world.find_or_assign_bit(TypeId::of::<$name>());
                    // Builds the `ComponentInfo` for the type, with that bit
                    ComponentInfo::of::<$name>(bit)
                }),+]
            }

            // Allows the variables below to take the type parameter names, which are in capitals
            #[allow(non_snake_case)]
            fn push_into(self, table: &mut Archetype, row: usize, existing: ComponentMask) {
                // Moves each field out of the tuple, into a variable with the name of its type
                let ($($name,)+) = self;
                // Stores each component at `row`, and keeps each replaced component in `replaced`
                let replaced = ($(push_or_replace(table, row, $name, existing),)+);
                // Drops the replaced components, now that every column has its new length
                drop(replaced);
            }
        }
    };
}

// Implements `Bundle` for every tuple of one to thirty-two components
impl_bundle!(A);
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);
impl_bundle!(A, B, C, D, E, F, G, H, I);
impl_bundle!(A, B, C, D, E, F, G, H, I, J);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
impl_bundle!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD,
    AE
);
impl_bundle!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD,
    AE, AF
);

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentVec;
    use crate::mask::ComponentMask;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, PartialEq)]
    struct Position(f32);

    impl Component for Position {}

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    impl Component for Health {}

    #[derive(Debug, PartialEq)]
    struct Velocity(f32);

    impl Component for Velocity {}

    /// Stands for a different component type at each value of `N`, which lets one test build a
    /// bundle of thirty-two types.
    #[derive(Debug, PartialEq)]
    struct Marker<const N: usize>(usize);

    impl<const N: usize> Component for Marker<N> {}

    /// Adds 1 to a shared counter when dropped, which shows how many times clean-up code ran.
    struct DropCounter {
        drops: Arc<AtomicUsize>,
    }

    impl Component for DropCounter {}

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Panics when dropped where `panics` is `true`, and drops quietly otherwise.
    struct Volatile {
        panics: bool,
    }

    impl Component for Volatile {}

    impl Drop for Volatile {
        fn drop(&mut self) {
            if self.panics {
                panic!("a `Volatile` with `panics` set was dropped");
            }
        }
    }

    /// Defines the bundle type with the most fields that `impl_bundle!` implements `Bundle` for.
    type Bundle32 = (
        Marker<0>,
        Marker<1>,
        Marker<2>,
        Marker<3>,
        Marker<4>,
        Marker<5>,
        Marker<6>,
        Marker<7>,
        Marker<8>,
        Marker<9>,
        Marker<10>,
        Marker<11>,
        Marker<12>,
        Marker<13>,
        Marker<14>,
        Marker<15>,
        Marker<16>,
        Marker<17>,
        Marker<18>,
        Marker<19>,
        Marker<20>,
        Marker<21>,
        Marker<22>,
        Marker<23>,
        Marker<24>,
        Marker<25>,
        Marker<26>,
        Marker<27>,
        Marker<28>,
        Marker<29>,
        Marker<30>,
        Marker<31>,
    );

    /// Builds an array with the `TypeId` of each type `Marker<N>`, for every `N` listed.
    macro_rules! marker_type_ids {
        ($($n:literal)*) => { [$(TypeId::of::<Marker<$n>>()),*] };
    }

    /// Builds an array with the `TypeId` of each array type `[u8; N]`, for every `N` listed.
    macro_rules! array_type_ids {
        ($($length:literal)*) => { [$(TypeId::of::<[u8; $length]>()),*] };
    }

    /// Returns a `Bundle32` whose field of type `Marker<N>` contains `N`.
    fn build_bundle_32() -> Bundle32 {
        (
            Marker(0),
            Marker(1),
            Marker(2),
            Marker(3),
            Marker(4),
            Marker(5),
            Marker(6),
            Marker(7),
            Marker(8),
            Marker(9),
            Marker(10),
            Marker(11),
            Marker(12),
            Marker(13),
            Marker(14),
            Marker(15),
            Marker(16),
            Marker(17),
            Marker(18),
            Marker(19),
            Marker(20),
            Marker(21),
            Marker(22),
            Marker(23),
            Marker(24),
            Marker(25),
            Marker(26),
            Marker(27),
            Marker(28),
            Marker(29),
            Marker(30),
            Marker(31),
        )
    }

    /// Returns 64 distinct `TypeId`s, enough to take every bit that a world can hand out.
    fn build_64_type_ids() -> [TypeId; 64] {
        array_type_ids!(
             0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
            16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
            32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
            48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63
        )
    }

    /// Returns a counter that adds to `drops` when dropped.
    fn build_counter(drops: &Arc<AtomicUsize>) -> DropCounter {
        DropCounter {
            drops: Arc::clone(drops),
        }
    }

    /// Returns the type and the bit of each entry in `infos`, in order.
    fn read_types_and_bits(infos: &[ComponentInfo]) -> Vec<(TypeId, ComponentMask)> {
        infos
            .iter()
            .map(|info| (info.type_id(), info.bit()))
            .collect()
    }

    /// Pushes `value` onto the column that stores `T`, which leaves the entity list unchanged.
    fn push_component<T: 'static>(table: &mut Archetype, value: T) {
        let column = table
            .components_of_mut(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.push_typed(value);
    }

    /// Returns the column that stores `T`, as a slice of `T`.
    ///
    /// The slice borrows `table`, meaning the compiler rejects any change to the table while the
    /// slice is in use.
    fn read_column<T: 'static>(table: &Archetype) -> &[T] {
        let column = table
            .components_of(TypeId::of::<T>())
            .expect("the table should store the component type");
        column.as_slice::<T>()
    }

    /// Checks that `infos` lists one entry per field in the order of the fields, and that a new
    /// world hands out its bits in that same order
    #[test]
    fn infos_with_new_types_in_any_order_returns_infos_in_field_order() {
        // Defines the type of `Bundle::infos` for one bundle type, which each case passes
        type InfosFn = fn(&mut World) -> Vec<ComponentInfo>;

        let cases: [(&str, InfosFn, [TypeId; 3]); 3] = [
            (
                "position, health, velocity",
                <(Position, Health, Velocity) as Bundle>::infos,
                [
                    TypeId::of::<Position>(),
                    TypeId::of::<Health>(),
                    TypeId::of::<Velocity>(),
                ],
            ),
            (
                "velocity, position, health",
                <(Velocity, Position, Health) as Bundle>::infos,
                [
                    TypeId::of::<Velocity>(),
                    TypeId::of::<Position>(),
                    TypeId::of::<Health>(),
                ],
            ),
            (
                "health, velocity, position",
                <(Health, Velocity, Position) as Bundle>::infos,
                [
                    TypeId::of::<Health>(),
                    TypeId::of::<Velocity>(),
                    TypeId::of::<Position>(),
                ],
            ),
        ];

        for (label, infos_of, type_ids) in cases {
            let mut world = World::new();

            let infos = infos_of(&mut world);

            let bits = [0, 1, 2].map(ComponentMask::single);
            let expected: Vec<(TypeId, ComponentMask)> = type_ids.into_iter().zip(bits).collect();
            assert_eq!(read_types_and_bits(&infos), expected, "infos for {label}");
        }
    }

    /// Checks that a type the world has met before keeps its bit, and a new type takes the next
    /// free bit
    #[test]
    fn infos_with_known_and_new_types_reuses_known_bits() {
        let mut world = World::new();
        world.find_or_assign_bit(TypeId::of::<Health>());
        world.find_or_assign_bit(TypeId::of::<Position>());

        let infos = <(Position, Velocity, Health) as Bundle>::infos(&mut world);
        let next_bit = world.find_or_assign_bit(TypeId::of::<Marker<0>>());

        let expected = vec![
            (TypeId::of::<Position>(), ComponentMask::single(1)),
            (TypeId::of::<Velocity>(), ComponentMask::single(2)),
            (TypeId::of::<Health>(), ComponentMask::single(0)),
        ];
        assert_eq!(read_types_and_bits(&infos), expected, "infos");
        assert_eq!(next_bit, ComponentMask::single(3), "next free bit");
    }

    /// Checks that a second call for the same bundle returns the same infos, and takes no new bits
    #[test]
    fn infos_with_repeated_call_returns_same_infos() {
        let mut world = World::new();
        let first = <(Position, Health) as Bundle>::infos(&mut world);

        let second = <(Position, Health) as Bundle>::infos(&mut world);
        let next_bit = world.find_or_assign_bit(TypeId::of::<Velocity>());

        assert_eq!(
            read_types_and_bits(&second),
            read_types_and_bits(&first),
            "infos of the second call"
        );
        assert_eq!(next_bit, ComponentMask::single(2), "next free bit");
    }

    /// Checks that a bundle that lists one type twice gets the same bit for both fields, which
    /// leaves the world to reject the bundle
    #[test]
    fn infos_with_repeated_type_returns_same_bit_twice() {
        let mut world = World::new();

        let infos = <(Health, Health) as Bundle>::infos(&mut world);

        let expected = vec![
            (TypeId::of::<Health>(), ComponentMask::single(0)),
            (TypeId::of::<Health>(), ComponentMask::single(0)),
        ];
        assert_eq!(read_types_and_bits(&infos), expected);
    }

    /// Checks the smallest bundle, of one field
    #[test]
    fn infos_with_one_field_returns_one_info() {
        let mut world = World::new();

        let infos = <(Health,) as Bundle>::infos(&mut world);

        let expected = vec![(TypeId::of::<Health>(), ComponentMask::single(0))];
        assert_eq!(read_types_and_bits(&infos), expected);
    }

    /// Checks the largest bundle, of thirty-two fields, with one bit per field in field order
    #[test]
    fn infos_with_32_fields_returns_32_infos_in_field_order() {
        let mut world = World::new();

        let infos = Bundle32::infos(&mut world);

        let type_ids = marker_type_ids!(
             0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
            16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
        );
        let expected: Vec<(TypeId, ComponentMask)> = type_ids
            .into_iter()
            .enumerate()
            .map(|(position, type_id)| (type_id, ComponentMask::single(position)))
            .collect();
        assert_eq!(read_types_and_bits(&infos), expected);
    }

    /// Checks that a new type still gets a bit where only the last bit is free
    #[test]
    fn infos_with_last_free_bit_assigns_bit_63() {
        let mut world = World::new();
        for type_id in &build_64_type_ids()[..63] {
            world.find_or_assign_bit(*type_id);
        }

        let infos = <(Health,) as Bundle>::infos(&mut world);

        let expected = vec![(TypeId::of::<Health>(), ComponentMask::single(63))];
        assert_eq!(read_types_and_bits(&infos), expected);
    }

    /// Checks that a world with every bit taken still describes a bundle of types that it has met
    #[test]
    fn infos_with_full_world_and_known_types_returns_known_bits() {
        let mut world = World::new();
        world.find_or_assign_bit(TypeId::of::<Position>());
        world.find_or_assign_bit(TypeId::of::<Health>());
        for type_id in &build_64_type_ids()[..62] {
            world.find_or_assign_bit(*type_id);
        }

        let infos = <(Health, Position) as Bundle>::infos(&mut world);

        let expected = vec![
            (TypeId::of::<Health>(), ComponentMask::single(1)),
            (TypeId::of::<Position>(), ComponentMask::single(0)),
        ];
        assert_eq!(read_types_and_bits(&infos), expected);
    }

    /// Checks that a 65th component type panics, since a mask has room for 64 bits
    #[test]
    #[should_panic]
    fn infos_with_65th_component_type_panics() {
        let mut world = World::new();
        for type_id in build_64_type_ids() {
            world.find_or_assign_bit(type_id);
        }

        <(Health,) as Bundle>::infos(&mut world);
    }

    /// Checks that a row at the end of the column, including row 0 of an empty column, takes a push
    /// and returns `None`
    #[test]
    fn push_or_replace_with_row_at_end_pushes_and_returns_none() {
        let cases = [0_u32, 1, 4];

        for count in cases {
            let mut table =
                Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
            for value in 0..count {
                push_component(&mut table, Health(value));
            }

            let replaced =
                push_or_replace(&mut table, count as usize, Health(99), ComponentMask::EMPTY);

            let mut expected: Vec<Health> = (0..count).map(Health).collect();
            expected.push(Health(99));
            assert_eq!(replaced, None, "returned component for {count} rows");
            assert_eq!(
                read_column::<Health>(&table),
                expected,
                "column for {count} rows"
            );
        }
    }

    /// Checks that a row inside the column takes the new component in place, at the first, a middle
    /// and the last row, and returns the old one
    #[test]
    fn push_or_replace_with_existing_row_replaces_and_returns_old_component() {
        let cases = [
            (0, [Health(99), Health(1), Health(2)]),
            (1, [Health(0), Health(99), Health(2)]),
            (2, [Health(0), Health(1), Health(99)]),
        ];

        for (row, expected) in cases {
            let mut table =
                Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);
            for value in 0..3 {
                push_component(&mut table, Health(value));
            }

            let replaced = push_or_replace(&mut table, row, Health(99), ComponentMask::single(0));

            assert_eq!(
                replaced,
                Some(Health(row as u32)),
                "returned component for row {row}"
            );
            assert_eq!(
                read_column::<Health>(&table),
                expected,
                "column for row {row}"
            );
        }
    }

    /// Checks that a component type that takes no memory is still pushed and replaced by count
    #[test]
    fn push_or_replace_with_zero_size_type_pushes_then_replaces() {
        #[derive(Debug, PartialEq)]
        struct Tag;

        impl Component for Tag {}

        let mut table = Archetype::new(&[ComponentInfo::of::<Tag>(ComponentMask::single(0))]);

        let pushed = push_or_replace(&mut table, 0, Tag, ComponentMask::EMPTY);
        let replaced = push_or_replace(&mut table, 0, Tag, ComponentMask::single(0));

        assert_eq!(pushed, None, "returned component for the push");
        assert_eq!(
            replaced,
            Some(Tag),
            "returned component for the replacement"
        );
        assert_eq!(read_column::<Tag>(&table).len(), 1, "column length");
    }

    /// Checks that a table with no column for the type panics
    #[test]
    #[should_panic]
    fn push_or_replace_with_missing_column_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Position>(ComponentMask::single(0))]);

        push_or_replace(&mut table, 0, Health(1), ComponentMask::EMPTY);
    }

    /// Checks that each component goes onto the end of its own column, whatever the field order
    #[test]
    fn push_into_with_empty_columns_pushes_each_component() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);

        (Health(7), Position(1.5)).push_into(&mut table, 0, ComponentMask::EMPTY);

        assert_eq!(
            read_column::<Position>(&table),
            [Position(1.5)],
            "positions"
        );
        assert_eq!(read_column::<Health>(&table), [Health(7)], "healths");
    }

    /// Checks that a bundle replaces the components at its row, and leaves the other rows alone
    #[test]
    fn push_into_with_existing_row_replaces_each_component() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[position, health]);
        for value in 0..3 {
            push_component(&mut table, Position(value as f32));
            push_component(&mut table, Health(value));
        }

        let existing = table.mask();

        (Position(9.0), Health(9)).push_into(&mut table, 1, existing);

        assert_eq!(
            read_column::<Position>(&table),
            [Position(0.0), Position(9.0), Position(2.0)],
            "positions"
        );
        assert_eq!(
            read_column::<Health>(&table),
            [Health(0), Health(9), Health(2)],
            "healths"
        );
    }

    /// Checks that one bundle pushes onto the columns that lack its row, and replaces at the row in
    /// the columns that have it, as after an entity moves to a larger table
    #[test]
    fn push_into_with_some_types_present_pushes_new_and_replaces_existing() {
        let position = ComponentInfo::of::<Position>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let velocity = ComponentInfo::of::<Velocity>(ComponentMask::single(2));
        let mut table = Archetype::new(&[position, health, velocity]);
        push_component(&mut table, Position(0.0));
        push_component(&mut table, Health(0));
        push_component(&mut table, Velocity(0.0));
        push_component(&mut table, Health(1));

        (Velocity(9.0), Health(9), Position(9.0)).push_into(&mut table, 1, health.bit());

        assert_eq!(
            read_column::<Position>(&table),
            [Position(0.0), Position(9.0)],
            "positions"
        );
        assert_eq!(
            read_column::<Health>(&table),
            [Health(0), Health(9)],
            "healths"
        );
        assert_eq!(
            read_column::<Velocity>(&table),
            [Velocity(0.0), Velocity(9.0)],
            "velocities"
        );
    }

    /// Checks that a replaced component is dropped once, and that a pushed or a new component stays
    /// alive until its table is dropped
    #[test]
    fn push_into_with_drop_counters_drops_each_replaced_component_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let counter = ComponentInfo::of::<DropCounter>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[counter, health]);
        push_component(&mut table, build_counter(&drops));

        (build_counter(&drops), Health(1)).push_into(&mut table, 0, counter.bit());
        let drops_after_push = drops.load(Ordering::Relaxed);
        drop(table);
        let drops_after_table = drops.load(Ordering::Relaxed);

        assert_eq!(drops_after_push, 1, "drops caused by `push_into`");
        assert_eq!(drops_after_table, 2, "drops after the table is gone");
    }

    /// Checks that a replaced component whose clean-up code panics leaves every column at its new
    /// length, since `push_into` drops it only once every component is stored
    #[test]
    fn push_into_with_panicking_replaced_component_leaves_every_column_at_new_length() {
        let volatile = ComponentInfo::of::<Volatile>(ComponentMask::single(0));
        let health = ComponentInfo::of::<Health>(ComponentMask::single(1));
        let mut table = Archetype::new(&[volatile, health]);
        push_component(&mut table, Volatile { panics: true });
        let bundle = (Volatile { panics: false }, Health(7));

        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            bundle.push_into(&mut table, 0, volatile.bit());
        }));

        let stored_panics: Vec<bool> = read_column::<Volatile>(&table)
            .iter()
            .map(|component| component.panics)
            .collect();
        assert!(outcome.is_err(), "outcome of `push_into`");
        assert_eq!(stored_panics, [false], "stored `Volatile` components");
        assert_eq!(read_column::<Health>(&table), [Health(7)], "healths");
    }

    /// Checks the smallest bundle, of one field, for both a push and a replacement
    #[test]
    fn push_into_with_one_field_pushes_then_replaces() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Health>(ComponentMask::single(0))]);

        (Health(1),).push_into(&mut table, 0, ComponentMask::EMPTY);
        (Health(2),).push_into(&mut table, 0, ComponentMask::single(0));

        assert_eq!(read_column::<Health>(&table), [Health(2)]);
    }

    /// Checks the largest bundle, of thirty-two fields, in a table built from its own infos
    #[test]
    fn push_into_with_32_fields_pushes_onto_every_column() {
        let mut world = World::new();
        let infos = Bundle32::infos(&mut world);
        let mut table = Archetype::new(&infos);

        build_bundle_32().push_into(&mut table, 0, ComponentMask::EMPTY);

        let lengths: Vec<Option<usize>> = infos
            .iter()
            .map(|info| table.components_of(info.type_id()).map(ComponentVec::len))
            .collect();
        assert_eq!(lengths, vec![Some(1); 32], "length of each column");
        assert_eq!(
            read_column::<Marker<0>>(&table),
            [Marker(0)],
            "first column"
        );
        assert_eq!(
            read_column::<Marker<31>>(&table),
            [Marker(31)],
            "last column"
        );
    }

    /// Checks that a table with no column for one of the bundle types panics
    #[test]
    #[should_panic]
    fn push_into_with_missing_column_panics() {
        let mut table = Archetype::new(&[ComponentInfo::of::<Position>(ComponentMask::single(0))]);

        (Position(1.0), Health(1)).push_into(&mut table, 0, ComponentMask::EMPTY);
    }
}
