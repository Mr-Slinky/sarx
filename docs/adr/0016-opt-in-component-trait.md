---
status: accepted
date: 2026-11-08
decision-makers: Kheagen Haskins
---

# Opt-In Component Trait

## Context and Problem Statement

Every component that `enki` stores must be `Send + Sync + 'static`. The bound `Send + Sync`
lets the world move its components between threads, and share them. The bound `'static` lets the
world keep a component for as long as it needs.

Before this decision, the code spelled out that bound in full at every place that accepted a
component type. Those places included `World::add_component`, `ComponentInfo::of`, and the tuple
implementations of `Bundle`. A reader therefore learned what a component is only by reading the
bound. A change to the bound also had to reach every copy.

A trait named `Component` would state the bound once. Each of those places would then take
`T: Component`.

A trait of that kind would also decide which types count as components. Two methods on `World`
show why that matters. `add_component` gives an entity one component, and `add_components` gives it
every component in a bundle. A caller can pass a tuple to `add_component` by mistake:

```rust
// Meant add_components, which would give the entity a Position and a Health
world.add_component(player, (Position(0.0), Health(100)));
```

A tuple of two components meets the bound. Under the bound alone, this line compiled. The entity
then had one component, of type `(Position, Health)`. A call to `world.get::<Health>(player)`
therefore returned `None`.

A caller can also leave out the value of a tuple struct:

```rust
// Meant Health(100)
world.add_component(player, Health);
```

The name of a tuple struct is also a function that builds one. `Health` here is that function, of
type `fn(i32) -> Health`. A function item meets the bound as well. Under the bound alone, this line
compiled too. The world then stored the function as a component of the entity.

How does `enki` decide which types are components?

## Decision Drivers

- The bound must be written once, so that a change to it reaches every place that accepts a
  component type.
- A mistake such as the two above must fail to compile, with an error message that identifies the
  type.
- The ECS is structure that other code builds on. Its first version follows the design that Sarx
  keeps. Later work extends that design rather than replacing it.
- The developer must discuss any library that the decision brings in before Sarx takes it on,
  against the questions that the README sets out.

## Considered Options

- The bound written out at each place, with no trait
- A `Component` trait with a blanket implementation
- A `Component` trait that each type implements by hand
- A `Component` trait that each type implements through a derive macro

## Decision Outcome

Chosen option: "A `Component` trait that each type implements by hand", because the compiler then
rejects both mistakes above, and the trait does not need a library.

`enki` defines the trait in `component.rs`, with the bound as its supertraits, and with no
blanket implementation:

```rust
pub trait Component: Send + Sync + 'static {}
```

The game implements it once for each component type:

```rust
struct Health(i32);

impl Component for Health {}
```

`World::add_component` and the tuple implementations of `Bundle` take `T: Component`. So do the
private helpers that they call. Only the types that the game marks implement `Component`. As a
result, the compiler rejects `add_component(player, (Position(0.0), Health(100)))` and
`add_component(player, Health)`.

`ComponentInfo::of` keeps `Send + Sync + 'static` as its own bound. A `ComponentInfo` describes the
raw bytes that one column of a table stores. Its tests build one for plain types, such as `u32` and
`String`. An `impl Component` for those types would make them components for the game as well. The
check on which types are components therefore happens where a caller adds a component, in `World`
and `Bundle`.

A derive macro would generate the `impl` line for each type. It remains open as a later addition.
Rust requires such a macro to be defined in a crate of its own, called a proc-macro crate. A
proc-macro crate generates code at compile time. Almost every derive macro is written with two
libraries, `syn` and `quote`. The developer must discuss those libraries before Sarx takes on the
macro.

### Consequences

- Good, because every method that accepts a component from a caller states the bound through
  `Component`.
- Good, because the compiler rejects a tuple, a function item, or any other type that the game has
  not marked as a component. The error message identifies the type.
- Good, because each component type has an `impl` of its own. That `impl` can later declare
  constants for the type, such as how the world stores it.
- Good, because each component type states in the code that it is a component.
- Bad, because each component type needs one more line.
- Bad, because the game cannot implement `Component` for a type from another crate, such as
  `String`. Rust's orphan rule lets a crate implement a trait for a type only where the crate
  defines the trait or the type. The game must wrap such a value in a struct of its own, such as
  `struct Name(String);`.
- Bad, because `ComponentInfo::of` still spells out the bound. A change to the bound must reach it
  as well as `Component`.

### Confirmation

A reader confirms this decision in `enki` with three checks:

- `component.rs` defines `Component` with `Send + Sync + 'static` as its supertraits, and with no
  blanket implementation.
- `World::add_component` and the tuple implementations of `Bundle` take `T: Component`.
  `ComponentInfo::of` is the one other place that spells out the bound for a component type.
- A call to `add_component` with a tuple fails to compile.

## Pros and Cons of the Options

### The Bound Written Out at Each Place, with No Trait

Each place that accepts a component type would keep `Send + Sync + 'static`, as the code did before
this decision.

- Good, because it does not need new code.
- Bad, because a change to the bound must reach every copy.
- Bad, because a tuple and a function item both meet the bound. As a result, both mistakes compile.

### A `Component` Trait with a Blanket Implementation

`enki` would define `Component` with the bound as its supertraits. It would also implement
`Component` for every type that meets the bound:

```rust
impl<T: Send + Sync + 'static> Component for T {}
```

- Good, because the bound lives in one place.
- Good, because a component type does not need an extra line.
- Bad, because a tuple and a function item both meet the bound. As a result, both mistakes compile.
- Bad, because Rust forbids a second `impl` of `Component` for any type. The trait therefore cannot
  later declare constants for one component type.

### A `Component` Trait That Each Type Implements by Hand

The Decision Outcome sets out this option.

- Good, because the compiler rejects every type that the game has not marked as a component.
- Good, because it does not need a library.
- Bad, because each component type needs one more line.
- Bad, because a type from another crate needs a wrapper struct.

### A `Component` Trait That Each Type Implements Through a Derive Macro

Each component type would be marked with `#[derive(Component)]`, and the macro would generate the
`impl`. Bevy appears to work this way.

- Good, because the compiler rejects every type that the game has not marked as a component.
- Good, because the attribute appears on the type, beside its other derives.
- Bad, because the macro needs a proc-macro crate of its own.
- Bad, because a derive macro is almost always written with `syn` and `quote`. Both libraries still
  await a discussion with the developer.
