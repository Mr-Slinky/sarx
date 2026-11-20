# Deferred Work

This file lists work that the developer has noted and put off, with no date set for it. An entry
stays here until the work is done, and then comes out.

The developer's TODO list, in `docs/local/todo.md`, sets the order in which Sarx is built. The entries below fall outside
that order. Each one is an improvement that came up along the way, such as in a review or an audit,
and that no step on the TODO list needs yet.

Each entry states four things, under four headings:

- what the code does today
- the change that would replace it
- the reason that the change waits
- the point at which the change is worth revisiting

The entries are grouped by crate.

## enki

### Edge cache for table moves

#### Today

An entity moves to another table each time that it gains or loses a component. To find the target
table, `World` asks its `ComponentRegistry` for the bit of the component type, which the registry
looks up in its `bits` map. `World` then builds the target mask, and looks it up in `indices`, which
returns the table.

Both maps use the default hasher of `HashMap`, SipHash. SipHash resists keys that an attacker
crafts to collide. A game has no such attacker, and SipHash is slower than a simpler hasher for keys
such as a `TypeId` or a `ComponentMask`.

#### The change

Each `Archetype` would store its edges. An edge records that adding a given type to the table leads
to table N, or that removing it leads to table M. A move would read the edge from the source table,
and would look up `indices` only the first time that it follows a new edge.

A faster hasher for both maps is a separate, smaller option. It would come either from a library or
from a hasher written by hand. A library goes to discussion first, as the
[Libraries](repo-conventions.md#libraries) convention sets out.

#### Why it waits

ADR 0007 estimates that the ECS moves around 33 entities on each step. At that rate, the two
lookups for each move take a negligible share of a step.

#### When to revisit

The change is worth revisiting once a timing of a simulation step shows that table moves take a
measurable share of it.

### More than 64 component types

#### Today

A `ComponentMask` stores one bit for each component type, in a `u64`. A world therefore has room for
64 component types. `ComponentRegistry::find_or_assign_bit` panics when it meets a 65th type. The
test `find_or_assign_bit_with_65th_type_panics` checks the panic.

Under ADR 0007, each state of an entity is a marker component, such as `Hungry`. Each marker type
takes a bit. Physics, AI, rendering and the states of villagers will together come close to 64.

#### The change

`ComponentMask` would store a `u128`, or an array such as `[u64; 2]`. Every operation on the bits
lives in `mask.rs`, meaning the code change stays inside that file. `World::MAX_COMPONENT_TYPES` is
defined as `ComponentMask::CAPACITY`, and changes along with it. The tests and the doc example that
count 64 types, in `mask.rs` and `world.rs`, would change their counts.

#### Why it waits

The game has a handful of component types so far, far below 64.

#### When to revisit

The change is worth revisiting once the game has about 40 component types.

### Compaction threshold for list columns

#### Today

The list column has not been built yet. Its design stores the values of every row in one buffer,
and gives each row its own start and length. Removing a row, moving it to another table, or
changing its length leaves a run of dead slots in the buffer. The column counts those slots, and
will compact once they reach half of the buffer. Compacting rewrites every live row in row order
and closes the gaps.

The developer chose half as an educated guess, since no system yet produces dead slots at a rate
that can be measured.

#### The change

The threshold would come from measurement. A lower threshold keeps the column smaller and its
passes faster, at the price of compacting more often. A rough estimate for the largest expected
column, villager parts, puts the trade as follows:

- 50,000 villagers with about 6 parts of 32 bytes each come to about 9.6 MB of live values.
- At half dead, the column takes about 19 MB. That sits at the top of the L3 cache of a mid-level
  laptop, which ADR 0021 puts at 8 to 24 MB, and a full pass then starts reading from main memory.
- One compaction copies the live values once, which takes about 0.5 to 1 ms of a 16.7 ms step.

A threshold of a quarter would cap that column at about 12.8 MB. The threshold could also differ
from column to column, since a column that only the hand uses never builds up many dead slots.

The same review would settle how the column compacts. Compacting into a new buffer restores row
order, but holds the old and new buffers at once. Compacting in place does not allocate, but leaves
the rows in their scrambled memory order.

#### Why it waits

The rate at which dead slots build up depends on systems that do not exist yet: deaths, dismember,
and how often villagers move between tables. Any threshold chosen before then would be a guess.

#### When to revisit

The change is worth revisiting once the benchmark that ADR 0021 requires runs with real villagers,
or once a timing of a simulation step shows a list column taking a measurable share of it.

## hermes

### More GLB files as test data

#### Today

The tests that read a real GLB file all read `assets/hand-model.glb`, the hand that the developer
exported from Blender. Every other test builds its bytes by hand. The hand file has one mesh with
one primitive, stores its indices as 16-bit numbers, and packs each attribute tightly in its own
buffer view.

#### The change

Each new GLB file that Sarx takes on would join the tests as it lands. A file that differs from the
hand in one of these ways would cover a path that only hand-built bytes reach today:

- indices stored as 32-bit numbers, which a mesh with more than 65,535 vertices needs
- a buffer view with a `byteStride`, which interleaves the attributes of each vertex
- more than one mesh, or more than one primitive in a mesh
- a file from an exporter other than Blender

#### Why it waits

Sarx has one model so far, the hand.

#### When to revisit

The change is worth revisiting each time that a new GLB file arrives under `assets/`.
