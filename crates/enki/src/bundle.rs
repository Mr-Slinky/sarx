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
//! A bundle of one component takes a comma after the component, as in `(Health(100),)`. Rust reads
//! `(Health(100))` as a `Health` in brackets, rather than as a tuple.
//!
//! A tuple of thirty-three or more components does not implement [`Bundle`]. The compiler rejects
//! such a tuple as the argument to `add_components`.

use crate::archetype::Archetype;
use crate::component::{Component, ComponentInfo};
use crate::world::World;
use std::any::TypeId;
use std::mem;

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
/// Code outside the crate cannot implement the trait. An implementation has to write `Archetype`
/// in the signature of [`Bundle::push_into`]. Code outside the crate cannot write a path to
/// `Archetype`. As a result, the tuple implementations in the crate are the only ones.
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
pub trait Bundle: Send + Sync + 'static {
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
    /// Each component goes into the column in `table` that stores its type. Where that column
    /// already contains a component at `row`, the new component replaces it, and the method drops the
    /// old one. Otherwise the method pushes the new component onto the end of the column.
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
    /// * `row`: the entity's row in `table`. Each column MUST contain either `row` components, for a
    ///   push, or more than `row`, for a replacement.
    ///
    /// # Panics
    ///
    /// Panics where `table` has no column for one of the types in the bundle. Also panics where the
    /// `Drop` implementation of a replaced component panics.
    ///
    /// # Examples
    ///
    /// The method has a `self` parameter, unlike [`Bundle::infos`]. A caller therefore calls it on
    /// the bundle itself, with the dot syntax.
    ///
    /// The example stores a bundle of a `Position` and a `Health` in `target`, a table with a
    /// `Position` column and a `Health` column. The entity has already moved into `target`, at row
    /// `row`. The entity had a `Health` before the move, and lacked a `Position`.
    ///
    /// ```ignore
    /// // Creates a tuple of a `Position` and a `Health`. The tuple implements `Bundle`
    /// let components: (Position, Health) = (Position(0.0, 0.0), Health(100));
    ///
    /// // Borrows the table that the entity moved into
    /// let target: &mut Archetype = &mut self.archetypes[target_index];
    ///
    /// // Pushes the `Position` onto the `Position` column, and replaces the entity's `Health`
    /// components.push_into(target, row);
    ///
    /// // `push_into` took `components` by value. The compiler therefore rejects any use of
    /// // `components` from here on
    /// ```
    #[doc(hidden)]
    fn push_into(self, table: &mut Archetype, row: usize);
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Stores `value` at row `row` of the column in `table` that stores `T`.
///
/// Where the column contains more than `row` components, `value` replaces the component at `row`, and
/// the function returns the replaced component. Otherwise the function pushes `value` onto the end
/// of the column, and returns `None`.
///
/// The caller owns the returned component, and decides when it drops.
///
/// # Panics
///
/// Panics where `table` has no column for `T`.
fn push_or_replace<T: Component>(table: &mut Archetype, row: usize, value: T) -> Option<T> {
    // Finds the column for `T`, and panics where the table has none
    let column = table
        .components_of_mut(TypeId::of::<T>())
        .expect("the table should have a column for every type in the bundle");

    // Replaces the component at `row`, where the column already contains one there
    if column.len() > row {
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
///     fn push_into(self, table: &mut Archetype, row: usize) {
///         // Moves the three fields out of the tuple, into the variables `A`, `B` and `C`
///         let (A, B, C,) = self;
///
///         // Stores each value at `row`, and keeps each replaced value in the tuple `replaced`
///         let replaced = (
///             push_or_replace(table, row, A),
///             push_or_replace(table, row, B),
///             push_or_replace(table, row, C),
///         );
///
///         // Drops the replaced values, now that every column has its new length
///         drop(replaced);
///     }
/// }
/// ```
///
/// The compiler works out the type parameter of each `push_or_replace` call from the value that
/// the call passes. The call `push_or_replace(table, row, A)` therefore runs with `T` as the type `A`.
///
/// In `push_into`, each type parameter name is also the name of a variable. The line
/// `let (A, B, C,) = self;` creates the variables `A`, `B` and `C`. The variable `A` contains the
/// field of type `A`. Rust looks up type names and variable names in separate namespaces. As a
/// result, `TypeId::of::<A>()` refers to the type, while `push_or_replace(table, row, A)` refers
/// to the variable.
///
/// The comma after the last name in `(A, B, C,)` changes the meaning only in the call with one
/// name. Rust reads `(A)` as the type `A` in brackets, while `(A,)` is a tuple with one field.
///
/// # Calls past thirty-two
///
/// A tuple of thirty-three components needs a new call, below the call for thirty-two. The new
/// call lists the same names as the call for thirty-two, followed by one more name, `AG`.
macro_rules! impl_bundle {
    ($($name:ident),+) => {
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
            fn push_into(self, table: &mut Archetype, row: usize) {
                // Moves each field out of the tuple, into a variable with the name of its type
                let ($($name,)+) = self;
                // Stores each component at `row`, and keeps each replaced component in `replaced`
                let replaced = ($(push_or_replace(table, row, $name),)+);
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
