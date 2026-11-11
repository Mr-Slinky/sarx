---
status: accepted
date: 2026-11-11
decision-makers: Kheagen Haskins
---

# Systems Declare Their Access

## Context and Problem Statement

A system is the code that updates one aspect of the game on each step. Movement, needs and
animation are three examples. Each system reads some component types and writes others.

`enki` already sets the rules for one query. A query is written as a type. `&T` reads the `T` of
each entity, and `&mut T` writes it. A tuple, such as `(&mut Position, &Velocity)`, asks for every
type in it at once. The world hands out a query through one of two methods:

- `World::query` accepts only a query that reads. It borrows the world through `&World`, meaning
  several read queries can be in use at once.
- `World::query_mut` accepts any query. It borrows the world through `&mut World`, meaning nothing
  else can use the world while the query is in use.

The compiler enforces both rules through the borrow of the world. A query that asks for one type
twice, and writes it at least once, panics in `check_permissions` before the world hands out
anything. The trait `ReadOnlyQuery` marks a query that only reads. It is sealed, meaning only `enki`
can implement it.

Each query also lists the type that it asks for in each field, paired with a `Permission` of `Read`
or `Write`. These lists are the read set and the write set of the query.

None of this says what a system is, or how a system receives its data. Every system will be written
to that shape, and resources, commands and events will follow it too.

ADR 0019 sets the scale target at 50,000 villagers. A step of 16.7 ms then has room for about a
dozen full passes over every villager on one core. The shape of a system decides how much of a
mid-level laptop's other cores those passes can use.

How does a system receive its data, and how do systems run alongside one another?

## Decision Drivers

- ADR 0019 needs many passes over 50,000 villagers in each step, which calls for every core.
- ADR 0007 already sends every change to the world through one gate, as a request.
- The ECS is structure that the rest of the game builds on. Changing the shape of a system later
  would change every system written by then.
- The `Permission` lists in `enki` already record what each query reads and writes.

## Considered Options

- Systems as ordinary functions that take `&World` or `&mut World`, called in a fixed order
- Systems that declare what they read and write, run by a scheduler
- Several worlds, each with a writer of its own

## Decision Outcome

Chosen option: "Systems that declare what they read and write, run by a scheduler", because it uses
every kind of parallelism that the other options use, and adds systems running side by side.

Parallelism can come from two places:

- **Within one system.** The system splits the rows of its tables across the cores. ADR 0011 already
  allows this.
- **Between systems.** Two systems run on different cores at the same time.

Each system declares the queries that it uses. The scheduler reads the `Permission` list of each
query. Two systems conflict where one of them writes a component type that the other reads or
writes. The scheduler runs two systems at the same time only where they do not conflict. A system
can still split its own work across the cores.

A system does not receive `&mut World`. It changes the structure of the world through commands.
Spawning, despawning, adding a component and removing one are all changes to that structure. The
commands reach the world through the gate that ADR 0007 sets out.

A system can declare several queries. Its queries MUST NOT conflict with each other. The rule is the
one that `check_permissions` applies inside one query, applied across all the queries of the
system together.

The query rules in `enki` stay as they stand. `World::query` and `World::query_mut` remain for code
that runs outside the scheduler, such as setting up a level, and for tests.

Systems are structure, which other code builds on. The scheduler, resources and commands therefore
come before the first system of the game. The hand step waits for them, rather than writing its
systems against `&mut World` and rewriting them once the scheduler exists.

Several points remain open, for the discussions that design each piece:

- the form that a declaration takes, such as parameter types in the signature of each system
- how the scheduler orders two systems that conflict
- the design of resources, commands and events
- a separate world for the renderer, under ADR 0005, which the next section sets out

### A Separate World for the Renderer

The simulation keeps one world. Splitting it by entity, or by component, would cost more than it
gains, as the pros and cons below set out.

The boundary between simulation and rendering in ADR 0005 is different. The renderer could keep a
world of its own, copied from the simulation after each step. The simulation would then run its
next step while the renderer draws the last one. That remains open as a later addition.

### Consequences

- Good, because a step can use every core in two ways at once. One system splits its rows across
  the cores, while another system that does not conflict with it runs beside it.
- Good, because the `Permission` lists that `enki` already builds are the declarations that the
  scheduler compares.
- Good, because a system that changes the structure of the world goes through commands, which
  ADR 0007 already requires.
- Good, because the developer learns how a scheduler works out which systems can run together.
- Bad, because a system cannot reach the whole world at will. It can use only what it declares.
- Bad, because two systems that touch the same component type still run one after the other. The
  heavy passes over villagers all touch `Position`, and the scheduler cannot overlap them.
- Bad, because a structural change waits until the commands are applied. A system that spawns an
  entity cannot query it in the same pass.
- Bad, because Sarx needs commands before the first system that spawns or despawns under the
  scheduler.

### Confirmation

The query rules can be confirmed in `enki` today, with three checks:

- `World::query` takes `&self`, and accepts only a `Q: ReadOnlyQuery`.
- `World::query_mut` takes `&mut self`.
- `check_permissions` panics on `(&mut Position, &Position)`.

Once the scheduler exists, a reader confirms the rest with two checks:

- No system takes `&mut World`.
- The scheduler runs two systems at once only where neither writes a type that the other declares.

## Pros and Cons of the Options

### Systems as Ordinary Functions, Called in a Fixed Order

Each system is a function that takes `&World` or `&mut World`. Sarx calls the systems one after the
other. hecs leaves scheduling to its caller in this way.

- Good, because a system is a plain function, which a caller can run from anywhere.
- Good, because each system can still split its rows across the cores.
- Bad, because nothing else runs while a system writes. Where a pass is short, or does not split
  well, the other cores wait.
- Bad, because a move to a scheduler later would change the signature of every system written by
  then.

### Systems That Declare What They Read and Write, Run by a Scheduler

Bevy, a Rust game engine, runs its systems this way.

- Good, because systems that do not conflict run at the same time.
- Good, because each system can still split its rows across the cores.
- Bad, because every system takes its data in the form that the scheduler understands.

### Several Worlds, Each with a Writer of Its Own

The game splits its entities across several worlds, and runs one writer in each world at a time.

The split can follow entities, such as one world for each village. An `Entity` handle then means
something only inside its own world. A villager that the hand carries to another village would be
despawned from one world and spawned again in the other, under a new handle. Every reference to the
old handle would break, and physics would have to check collisions across the boundary.

The split can instead follow components, such as one world for physics and one for the AI. Each
villager then exists in several worlds, under handles that have to be kept in step. A system that
reads `Position` from physics and `Task` from the AI borrows both worlds.

- Good, because each world has a writer of its own, which runs beside the writers of the others.
- Bad, because the boundary between worlds is fixed when the code is written. A scheduler works out
  conflicts for each system, by component type.
- Bad, because an entity that crosses a boundary loses its handle.
- Bad, because the parallelism that a split by entity gives is the same that a system gains by
  splitting its rows across the cores.
