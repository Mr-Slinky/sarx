---
status: accepted
date: 2026-11-08
decision-makers: Kheagen Haskins
---

# Archetype Component Storage

## Context and Problem Statement

ADR 0009 settles that the developer writes the ECS by hand, in the `enki` crate. That record
leaves one question open: how the ECS stores its components. This record settles it.

The layout of the components in memory decides how fast the systems run. The reason lies in how the
CPU reads memory.

The CPU reads memory in 64-byte blocks, called cache lines. It keeps recently read lines in a small,
fast memory, called the cache. A read from the cache takes about 1 nanosecond. A read from main
memory takes about 100 nanoseconds.

The CPU also fetches lines ahead of time. When a loop walks forward through memory in order, the CPU
fetches the next lines before the loop asks for them. A loop that reads an array from start to finish
therefore rarely waits on main memory. A loop that jumps between scattered addresses waits on almost
every read.

Data-oriented design builds on these facts. A hot loop is a loop that runs often enough to decide
the speed of the program. Data-oriented design lays data out in the order in which the hot loops
read it.

In Sarx, the hot loops are the systems that run on every step over thousands of entities. Movement
and the physics engine are two of them. Each of these systems reads several components of each
entity together. The physics engine, for example, reads a position, a velocity, a mass and a
collider.

Entities also gain and lose components while the game runs. The central AI, which ADR 0006 sets
out, gives a villager a task and later takes it away. A villager becomes hungry, then eats.

How does the ECS lay out its components in memory?

## Decision Drivers

- The systems that run on every step read several components of each entity together. They should
  read memory in order.
- Entities gain and lose components while the game runs. The central AI assigning a task is one
  example.
- Some systems work on a whole column at once. Such a system uses slice operations, or SIMD
  instructions, which process several values in one CPU instruction.
- The ECS is structure that other code builds on. Its first version follows the design that Sarx
  keeps. Later work extends that design rather than replacing it.

## Considered Options

- Archetypes
- Sparse sets
- Archetypes, with sparse-set storage as an option for each component type
- One `Vec<Option<T>>` per component type, indexed by entity

## Decision Outcome

Chosen option: "Archetypes", because a query over any set of components then reads every table in
order, with no lookup for each entity.

An archetype is one set of component types, such as `{Position, Velocity}`. Each archetype has a
table. Every entity with exactly that set of types is stored in that table.

The table has one column for each of its component types. Row 5 of every column belongs to the same
entity.

A query asks for every entity with certain components, such as every entity with `Position` and
`Velocity`. The ECS answers it in two steps. It finds each table whose set includes both types. It
then walks the two columns of that table side by side.

A state that an entity enters and leaves is a component of its own. A hungry villager, for example,
has a `Hungry` component. Hungry villagers and fed villagers therefore belong to two different
archetypes, each with a table of its own:

```
Archetype {Position, Velocity}            fed villagers
Archetype {Position, Velocity, Hungry}    hungry villagers
```

A system that moves hungry villagers towards food walks the second table alone. It reads the
`Position` and `Hungry` columns in order.

Sparse sets, a second way to store components, remain open as a later extension. The developer
would add them for one component type at a time. They would add them only where a measurement shows that
moving entities between tables has become the slow part of a step.

### Queries per Table

A query hands a system one table at a time. For each matching table, the system receives one slice
for each component type that it asks for. A query for `(&mut Position, &Velocity)`, for example,
gives the system a `&mut [Position]` and a `&[Velocity]` for each table. Row 5 of each slice belongs
to the same entity.

The system then writes its own loop over the rows. It can also treat each slice as a whole, through
slice operations or SIMD instructions.

A second layer serves the systems that work on one entity at a time. It walks the slices of each
table, and yields one row at a time. Its loop reads memory in the same order as a loop over the
slices.

The slices form the base because only that direction works. A row iterator takes a few lines of code
on top of the slices. A system that receives one row at a time has no way to recover the slice that
the row came from.

flecs, an ECS written in C, appears to hand out queries per table in the same way. Unity's DOTS
hands out chunks of rows. hecs and Bevy lead with an iterator that yields one entity at a time.

### Consequences

- Good, because every query reads its tables in order. Row by row, the columns of a table line up
  on the same entity. A query therefore does not look anything up.
- Good, because every query that a system writes reads in order, whichever component types it asks
  for.
- Good, because a system can process a whole column at once, through slice operations or SIMD
  instructions.
- Good, because a later scheduler can split the tables of one query across threads.
- Good, because the developer learns how a type that is known only at run time is laid out in
  memory. Each column of a table is raw bytes, laid out with the size and alignment of its component
  type. `enki` reads those bytes through `unsafe` code.
- Bad, because adding a component to an entity, or removing one, moves the entity to another table.
  The move takes three steps. The ECS copies every component of the entity into the new table. It
  fills the old row with the last row of the table. It then updates the recorded location of the
  entity whose row moved.
- Bad, because every state component doubles the number of archetypes that an entity can belong to.
  Villagers with `Hungry`, `Tired` and `Carrying` spread over eight tables. A query over every
  villager then walks eight short runs in place of one long one.
- Bad, because the number of component types in one world has a limit. The ECS records the set of
  types in each table as a mask, with one bit for each type. A mask has a fixed number of bits,
  which `ComponentMask::CAPACITY` in `enki` defines. Every state component takes one of those
  bits, alongside the components of physics, the central AI and the villagers themselves.
- Bad, because the compiler cannot check the `unsafe` code in the tables. A mistake in the size or
  alignment of a column corrupts memory, where safe code would fail to compile.
- Bad, because the ECS builds each slice from a raw pointer and a row count, in `unsafe` code. A
  wrong count gives the system a slice that runs past the end of its column.

The number of moves is an estimate. Suppose each of 10,000 villagers changes state about once every
five seconds. At 60 steps a second, the ECS then moves around 33 entities on each step. That is
small work beside a loop over 10,000 entities. The rate of state changes is an assumption, and a
measurement replaces it once the central AI exists.

### Confirmation

A reader confirms this decision in `enki`. There, the ECS stores the components of each entity
in the table of its archetype, with one column per component type. A query there hands a system one
slice per component type for each table, and the row iterator builds on those slices.

## Pros and Cons of the Options

### Archetypes

The ECS keeps one table for each set of component types. It stores each entity in the table that
matches its set.

- Good, because a query over several component types reads each table in order, with no lookup for
  each entity.
- Good, because every query gains that speed, whichever component types it reads.
- Bad, because adding or removing a component copies the whole entity into another table.
- Bad, because many combinations of components spread entities over many small tables.

### Sparse Sets

The ECS keeps one store for each component type. A store has three arrays:

- a packed array of the components
- an array of the entity that owns each component
- a sparse array that maps the index of an entity to its slot in the packed array

EnTT, a widely used C++ ECS, appears to store its components this way.

- Good, because adding or removing a component changes one store. Every other component of the
  entity stays where it is.
- Good, because a system that reads one component type walks its packed array in order.
- Bad, because a query over two component types walks one packed array. For each entity there, it
  looks the entity up in the other store. Removals shuffle each store in a different order. After
  that, those lookups land at scattered addresses.
- Bad, because the fix for those lookups has a limit. EnTT calls the fix a group, and a group keeps a
  chosen set of stores in the same order. As far as the author knows, a component type belongs to
  one group at most. As a result, only the queries chosen ahead of time read in order.

### Archetypes, with Sparse-Set Storage as an Option for Each Component Type

The ECS stores components in archetype tables. A component type can choose a sparse set instead. The
ECS of Bevy appears to work this way.

- Good, because a component that entities gain and lose often does not move an entity between
  tables.
- Bad, because a query that reads both kinds of storage looks up the sparse-set components of each
  entity. Those lookups land at scattered addresses.
- Bad, because archetypes alone already serve a state such as `Hungry` with no lookup at all. The
  estimated number of moves between tables is also small.

### One `Vec<Option<T>>` per Component Type, Indexed by Entity

The ECS keeps one `Vec` for each component type. Slot `i` stores the component of the entity with
index `i`, or `None`.

- Good, because each store is a plain generic `Vec`, written in safe Rust.
- Bad, because every store has a slot for every entity. A component that few entities have leaves
  most of its store empty. A loop over that store steps through every empty slot.
- Bad, because the full ECS would replace it. The ECS is structure that Sarx builds properly from its
  first version.
