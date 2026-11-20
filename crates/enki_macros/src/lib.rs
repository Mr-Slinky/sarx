//! Defines the derive macros that `enki` re-exports: [`Component`](macro@Component) and
//! [`ListComponent`](macro@ListComponent).
//!
//! Each derive writes the one empty `impl` line that marks a type for `enki`. A caller reaches both
//! through `enki` itself, which re-exports each derive under the same name as its trait. Rust keeps
//! traits and macros in separate namespaces, so one `use` brings in both:
//!
//! ```ignore
//! use enki::Component;
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Component)]
//! pub struct Position(pub(crate) Vec3);
//! ```
//!
//! Rust requires a derive macro to live in a crate of its own, a proc-macro crate, which the
//! compiler runs while it compiles the crates that use the macro.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Path, parse_macro_input, parse_quote};

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Implements `enki::Component` for a struct or an enum, which a `World` can then store on an
/// entity.
///
/// The derive writes `impl enki::Component for Type {}`, and carries over any generic parameters,
/// their bounds and any `where` clause.
///
/// # Examples
///
/// ```ignore
/// use enki::Component;
///
/// #[derive(Debug, Component)]
/// struct Health(u32);
/// ```
#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    build_marker_impl(&input, parse_quote!(::enki::Component))
}

/// Implements `enki::ListComponent` for a struct or an enum, which a `World` can then store as one
/// element in a list on an entity.
///
/// The derive writes `impl enki::ListComponent for Type {}`, and carries over any generic
/// parameters, their bounds and any `where` clause.
///
/// # Examples
///
/// ```ignore
/// use enki::ListComponent;
///
/// #[derive(Clone, Copy, Debug, PartialEq, ListComponent)]
/// struct Bone {
///     rotation: Quat,
///     goal: Quat,
/// }
/// ```
#[proc_macro_derive(ListComponent)]
pub fn derive_list_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    build_marker_impl(&input, parse_quote!(::enki::ListComponent))
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// Returns an empty `impl` of the marker trait at `trait_path` for the type that `input` declares.
fn build_marker_impl(input: &DeriveInput, trait_path: Path) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let output = quote! {
        impl #impl_generics #trait_path for #name #type_generics #where_clause {}
    };
    output.into()
}
