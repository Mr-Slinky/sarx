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
the target table, `World` looks up the component type in `component_bits`, which returns its bit.
`World` then builds the target mask, and looks it up in `indices`, which returns the table.

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
