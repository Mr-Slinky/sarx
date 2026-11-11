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

None of this says what a system is, or how a system receives its data. The answer fixes the shape
of every system, and resources, commands and events will follow the same shape.

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

- Within one system, the system splits the rows of its tables across the cores, as ADR 0011 already
  allows.
- Between systems, two systems run on different cores at the same time.

Each system declares the queries that it uses, and the scheduler reads the `Permission` list of each
one. Two systems conflict where one of them writes a component type that the other reads or writes.
The scheduler runs two systems at the same time only where they do not conflict. A system can still
split its own work across the cores.

A system does not receive `&mut World`. Rather, it changes the structure of the world through
commands. Spawning, despawning, adding a component and removing one are all changes to that
structure. The commands reach the world through the gate that ADR 0007 sets out.

A system can declare several queries. Its queries MUST NOT conflict with each other. The rule is the
one that `check_permissions` applies inside one query, applied across all the queries of the
system together. The sections below set out the form that a declaration takes, the order in which
conflicting systems run, the threads that run them, and the resources that they share.

The query rules in `enki` stay as they stand. `World::query` and `World::query_mut` remain for code
that runs outside the scheduler, such as setting up a level, and for tests.

Systems are structure, which other code builds on. The scheduler, resources and commands therefore
come before the first system of the game. The hand step, the TODO item that draws the god's hand,
waits for them. Its systems must be written under the scheduler from the start, rather than against
`&mut World` first and rewritten once the scheduler exists.

Several points remain open, for the discussions that design each piece:

- the design of commands and events
- a separate world for the renderer, under ADR 0005, which the next section sets out

### A Separate World for the Renderer

The simulation keeps one world. Splitting it by entity would break the handles of entities that
cross the split, and splitting it by component would leave each villager in several worlds, as the
pros and cons below set out.

The boundary between simulation and rendering is another matter, since ADR 0005 already draws it.
The renderer could keep a
world of its own, copied from the simulation after each step. The simulation would then run its
next step while the renderer draws the last one. That remains open as a later addition.

### How a System Declares Its Access

Each system must be an ordinary Rust function, and its parameter types must declare its access.
Bevy declares access in the same way. A movement system, for example, would take one query:

```rust
fn movement(mut bodies: Query<(&mut Position, &Velocity)>) {
    for (position, velocity) in bodies.iter_mut() {
        position.0 += velocity.0;
    }
}
```

Each parameter type must implement one trait, called `SystemParam` here. The trait has two jobs. It
adds the entries for its type to the `Permission` list of the system, which the scheduler compares.
It also builds the parameter from the world each time the system runs. `Query` would implement it
by calling `add_permissions` for its query type. Resources and commands would take the same shape,
as further parameter types such as `Res<Clock>` and `Commands`.

The scheduler must accept any function whose parameters all implement `SystemParam`. It reads the
permissions of the function once, when the function is added. It then stores the function behind
one shared type, such as `Box<dyn System>`, beside every other system.

A parameter such as `Query<'w, Q>` borrows the world for one run of the system. The scheduler
creates a new borrow on every run. As a result, the function must accept a parameter for any
lifetime `'w`, which Rust states as a higher-ranked bound, `for<'w>`.

This form was chosen because the parameter list is both the declaration and the only route to the
world. The scheduler reads the access from the same types that the system uses. The declaration and
the code of the system therefore cannot disagree. A system that disagreed with its declaration could
write a component while another system reads it, which is a data race.

Two other forms were considered:

- A `System` trait, with the parameter as an associated type, such as `type Data<'w>`. It gives the
  same guarantee, and needs a struct and an `impl` for each system. In return, a system that keeps
  state between runs has a place for it in `self`. A later parameter type can give a function
  system that place instead.
- A list of reads and writes, given to the scheduler apart from the function. The function would
  reach the world through a general handle, and nothing would stop it from touching a type outside
  its list. Only a check at runtime on every access would make that safe.

### How the Scheduler Orders Conflicting Systems

The scheduler must keep its systems in one list, in the order that they were added. Where two
systems conflict, the one earlier in the list runs first. Systems that do not conflict run whenever
a worker is free, wherever they sit in the list.

```rust
scheduler.add_system(input);
scheduler.add_system(movement);
scheduler.add_system(collision);
scheduler.add_system(needs);
```

In the list above, movement and collision both write `Position`, and movement therefore runs first.
`needs` conflicts with neither, and can run beside either one.

At startup, the scheduler must log every pair of conflicting systems, with the order that the list
gives the pair. A system that gains a write, such as `needs` starting to write `Position`, also
gains a conflict. That conflict then appears as a new line in the log, rather than taking an order
unnoticed.

This form was chosen because Sarx has one schedule, which one file can set out in full. The place of
a system in the list states its order against every system that it conflicts with.

The other form considered was an explicit order on each system, such as `collision.after(movement)`,
with the scheduler refusing to start where a conflicting pair has none. That form forces a decision
on each new conflict, where the list chooses an order silently. Both forms give the scheduler the
same graph, with an arrow from each system to every system that must follow it. Under the list, the
scheduler draws the arrows from the positions in the list. As a result, `.after` can come later, on
top of list order, without changing any system.

How the scheduler turns the list into a running step stays inside the scheduler, and is settled when
the scheduler is built. That covers how it tracks which systems are ready, and which ready system a
free worker takes first.

### Threads

The scheduler must own every thread that runs a system. It creates a fixed set of worker threads
once, at startup, about one for each core, and keeps them for the life of the game. A system is a
job that runs on these threads, rather than a thread of its own. With hundreds of systems and four
workers, at most four systems run at once.

The scheduler and the workers share one queue of jobs. The scheduler places each system on the
queue once every system that must run before it has finished. Each idle worker takes the next job
from the queue, runs it, then returns for another. The main thread joins the workers while it waits
for a step to finish.

A system splits its rows by calling a method such as `par_iter_mut` in place of `iter_mut`. Such a
system cuts its columns into chunks, places the chunks on the same queue, and works through them
itself. Idle workers take the chunks that remain. Where every worker is busy, the thread that runs
the system works through every chunk alone. The number of threads that help a system is therefore
decided while it runs, from how busy the workers are at that moment.

Splitting never creates a conflict, since each chunk is a separate part of a column that the system
already declared. A system MUST wait for every chunk to finish before it returns, since each chunk
borrows the world through the borrow that the system holds. `std::thread::scope` applies the same
rule to scoped threads.

A system that started threads of its own would compete with the workers for the same cores. Every
split therefore goes through the queue.

### Resources

A resource is one value that the whole game shares, rather than one value for each entity. The
clock, the input and the level are resources, and so is the point on the ground under the cursor,
which the hand follows.

The world must store its resources in a map from the `TypeId` of each resource type to its value.
Each value is a `Box<dyn Any + Send + Sync>`, which the world downcasts to the type that a caller
asks for. A world holds at most one resource of each type. Code outside the scheduler, such as the
setup of a level, inserts and reads resources through the world, in the way that it reaches
entities.

A system reads a resource through a `Res<T>` parameter, and writes one through a `ResMut<T>`
parameter. Each implements `SystemParam`, and adds a read or a write entry to the `Permission` list
of the system:

```rust
fn steer_hand(target: Res<CursorTarget>, mut hands: Query<(&mut Velocity, &Position)>) {
    // ...
}
```

Two systems conflict over a resource in the way that they conflict over a component type: where one
of them writes the resource, and the other reads or writes it. Each `Permission` entry must record
whether it refers to a component or a resource. A type that is both a component and a resource then
produces a conflict only where two systems use it in the same role.

Before the first step, the scheduler must check that the world holds every resource that its systems
ask for. Where one is missing, the scheduler refuses to start, and names the system and the missing
type.

Resources live in the world because `SystemParam` builds every parameter from the world. Systems and
the code outside the scheduler then reach components and resources through one value. Two other
homes were considered:

- A single entity that holds the resource as a component. Queries and the conflict check would work
  with no new code. Nothing would keep the entity unique, and each read would search the tables as a
  query does.
- The scheduler, apart from the world. The setup of a level and the tests run without the scheduler,
  and would have to reach resources by a second route.

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
- Bad, because the list gives every new conflict an order without asking. The startup log is the
  only signal that a conflict has appeared.

### Confirmation

The query rules can be confirmed in `enki` today, with three checks:

- `World::query` takes `&self`, and accepts only a `Q: ReadOnlyQuery`.
- `World::query_mut` takes `&mut self`.
- `check_permissions` panics on `(&mut Position, &Position)`.

Once the scheduler exists, a reader confirms the rest with seven checks:

- No system takes `&mut World`.
- Every system is a function, and each of its parameter types implements `SystemParam`.
- The startup log lists each pair of conflicting systems, in the order that the list gives it.
- The scheduler creates its worker threads once, and no system starts a thread of its own.
- A system reaches a resource only through `Res<T>` or `ResMut<T>`.
- The scheduler refuses to start where a system asks for a resource that the world does not hold.
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
