---
status: accepted
date: 2026-11-10
decision-makers: Kheagen Haskins
---

# Change Detection

## Context and Problem Statement

Change detection tells a system which components have changed since that system last ran. A system
that reacts to changes can then skip every component that stayed the same. A system that redraws
health bars, for example, would redraw only the bars of villagers whose `Health` changed.

Each system measures change from its own last run. The renderer last ran one frame ago, and an AI
system might have last run ten steps ago. One `Health` component can therefore count as changed for
one system and unchanged for another.

`enki` cannot detect changes today. A `&mut T` query hands each system a plain `&mut T`. A write
through a `&mut T` is a plain store to memory. No code in `enki` runs when the system writes. As a
result, the world never learns which components the system changed.

A record of changes must therefore come from one of three places:

- At each write, the query would hand out a wrapper, such as `Mut<'w, T>`, in place of `&'w mut T`.
  The wrapper would implement `DerefMut`, the trait whose method runs whenever code takes a
  `&mut T` through the wrapper, as every write does. That method would stamp the current step
  number beside the component.
- At each loan, the world would stamp a whole column with the current step number when it lends
  the column for writing. The world could not tell which rows were written, or whether any were.
- At the writer, the system that changes a component knows which components it changed. That
  system would report them itself, as events or as a list of entities.

Only the first would change what a query hands out. Adding it later would change the item type of
every `&mut T` query. In turn, it would change every system written by then. As a result, this
decision has to come before the first systems, whatever the answer.

How does Sarx learn which components changed?

## Decision Drivers

- ADR 0019 sets the target at 50,000 villagers. A full pass over their components reads about
  10 MB, and takes about 0.5 to 1 ms.
- Systems that write every villager on every step, such as movement, needs and animation, take most
  of each step.
- Some reactions are sparse. In a sparse reaction, a few villagers out of thousands change, and the
  work for each change is large. Such reactions will be common in a large game.
- Under ADR 0011, each table stores its components as whole columns, and a system can work on one
  column at a time.
- Under ADR 0011, a state is also a marker component, such as `Hungry`. A change of state moves the
  entity to another table, and a query for the marker finds it there.

## Considered Options

- A step number for every component, stamped at each write
- A step number for every column, stamped at each loan
- No tracking in `enki`, with events from the writer for sparse changes

## Decision Outcome

Chosen option: "No tracking in `enki`, with events from the writer for sparse changes", because it
does not add any work to the systems that write every villager, and the work of its readers grows
with the number of changes, rather than with the number of villagers.

A `&mut T` query keeps `&'w mut T` as its item, and `query_archetype_mut` keeps `&'w mut [T]` as its
column.

Each kind of change reaches its readers by its own route. The table below gives the route for each
kind.

| Kind of change                                   | Route                                                    |
|--------------------------------------------------|----------------------------------------------------------|
| Every row, on every step, such as movement       | none; the reader processes every row, as it would anyway |
| A few rows, and the writer knows which           | an event, or a list of changed entities, from the writer |
| A change of state, such as becoming `Hungry`     | a marker component, under ADR 0011                       |

Events do not exist yet. Their design is a later decision.

The renderer does not need change detection at this scale. At 64 bytes for each transform, the
15,000 villagers on screen come to about 1 MB of transforms for each frame. The renderer can send
every visible transform on every frame.

A step number for every column remains open as a later addition. It would add to the API, and would
leave every existing signature as it is. A later system that needs it, such as a renderer that skips
tables that nobody changed, can bring it in at that point.

### Consequences

- Good, because the systems that write every villager write nothing beyond their own components. At
  50,000 villagers, the memory traffic that they avoid leaves time for more systems in each step.
- Good, because a reader of sparse changes reads only those changes. Five damaged villagers mean
  five entries to read, whatever the population.
- Good, because a system keeps a plain `&mut T`, which it can pass to any function that takes one.
- Good, because a step number for every column can still come later, without changing any system.
- Bad, because a writer must remember to report each sparse change. A writer that forgets leaves
  every reader out of date, and nothing in `enki` detects the omission.
- Bad, because Sarx needs an event system before the first sparse reaction.

### Confirmation

A reader confirms this decision in `enki` with two checks:

- `<&mut T as Query>::Item<'w>` is `&'w mut T`, and `<&mut T as Query>::Columns<'w>` is
  `&'w mut [T]`.
- No column and no component stores a step number.

## Pros and Cons of the Options

### A Step Number for Every Component, Stamped at Each Write

Bevy, a Rust game engine, tracks changes this way.

- Good, because only a component that a system wrote counts as changed.
- Good, because a writer cannot forget to report a change.
- Bad, because every write also writes a step number. For a 12-byte `Position` beside a 4-byte step
  number, every write moves about a third more memory. The extra writes happen in the systems that
  write every villager on every step, and those systems take most of each step.
- Bad, because a reader still compares one step number for every row. Finding five changed
  villagers means comparing 50,000 step numbers.
- Bad, because the item type changes to a wrapper. Code that passes the item to a function that
  takes `&mut T`, or matches on its type, must change.

### A Step Number for Every Column, Stamped at Each Loan

Unity's ECS for large entity counts tracks changes in a similar way, with one number for each
fixed-size chunk of a table.

- Good, because it adds one write for each column that a query lends, rather than one for each row.
- Good, because a reader can skip a whole table that nobody wrote to, such as a table of trees.
- Good, because it adds to the API, and leaves every existing signature as it is.
- Bad, because it counts a loan as a change. The world lends the `Position` column of the villagers
  to movement on every step. That column is therefore always marked as changed.
- Bad, because it cannot say which rows changed.

### No Tracking in `enki`, with Events from the Writer

- Good, because a write stays a plain store to memory.
- Good, because the work of a reader grows with the number of changes.
- Bad, because each writer must report its own changes.
