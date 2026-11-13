# Deferred Work

This file lists work that has been noted and put off, with no date set for it. An entry stays here
until the work is done, and is deleted once it is.

The TODO section of the README sets the order in which Sarx is built. The entries below sit outside
that order. Each one is an improvement that came up along the way, such as in a review or an audit,
and that no step on the TODO list needs yet.

Each entry states four things:

- what the code does today
- the change that would replace it
- the reason that the change waits
- the point at which the change is worth revisiting

The entries are grouped by crate.

## enki

### Edge cache for table moves

**Today**: an entity moves to another table each time that it gains or loses a component. To find
the target table, `World` asks its `ComponentRegistry` for the bit of the component type, which the
registry looks up in its `bits` map. `World` then builds the target mask, and looks it up in
`indices`, which returns the table.

Both maps use the default hasher of `HashMap`, SipHash. SipHash resists keys that an attacker
crafts to collide. A game has no such attacker, and SipHash is slower than a simpler hasher for keys
such as a `TypeId` or a `ComponentMask`.

**The change**: each `Archetype` would store its edges. An edge records that adding a given type to
the table leads to table N, or that removing it leads to table M. A move would read the edge from
the source table, and would look up `indices` only the first time that it follows a new edge.

A faster hasher for both maps is a separate, smaller option. It would come either from a library or
from a hasher written by hand. A library goes to discussion first, as the README sets out.

**Why it waits**: ADR 0011 estimates that the ECS moves around 33 entities on each step. At that
rate, the two lookups per move take a negligible share of a step.

**When to revisit**: once a timing of a simulation step shows that table moves take a measurable
share of it.

### More than 64 component types

**Today**: a `ComponentMask` stores one bit for each component type, in a `u64`. A world therefore
has room for 64 component types. `ComponentRegistry::find_or_assign_bit` panics when it meets a
65th type. The test `find_or_assign_bit_with_65th_type_panics` checks the panic.

Under ADR 0011, each state of an entity is a marker component, such as `Hungry`. Each marker type
takes a bit. Physics, AI, rendering and the states of villagers will together come close to 64.

**The change**: `ComponentMask` would store a `u128`, or an array such as `[u64; 2]`. Every
operation on the bits lives in `mask.rs`, meaning the code change stays inside that file.
`World::MAX_COMPONENT_TYPES` is defined as `ComponentMask::CAPACITY`, and changes along with it. The
tests and the doc example that count 64 types, in `mask.rs` and `world.rs`, would change their
counts.

**Why it waits**: the game has a handful of component types so far, far below 64.

**When to revisit**: once the game has about 40 component types.

## hermes

### More GLB files as test data

**Today**: the tests that read a real GLB file all read `assets/hand-model.glb`, the hand that the
developer exported from Blender. Every other test builds its bytes by hand. The hand file has one
mesh with one primitive, stores its indices as 16-bit numbers, and packs each attribute tightly in
its own buffer view.

**The change**: each new GLB file that Sarx takes on would join the tests as it lands. A file that
differs from the hand in one of these ways would cover a path that only hand-built bytes reach
today:

- indices stored as 32-bit numbers, which a mesh with more than 65,535 vertices needs
- a buffer view with a `byteStride`, which interleaves the attributes of each vertex
- more than one mesh, or more than one primitive in a mesh
- a file from an exporter other than Blender

**Why it waits**: Sarx has one model so far, the hand.

**When to revisit**: each time that a new GLB file arrives under `assets/`.
