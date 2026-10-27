---
status: accepted
date: 2026-10-17
decision-makers: Kheagen Haskins
---

# A Hand-Built ECS

## Context and Problem Statement

Sarx simulates a world full of objects, such as villagers, trees and buildings. Many of these
objects share data and behaviour. A villager and a thrown rock, for example, both have a position.
Gravity also acts on both.

An entity component system (ECS) is one way to organise those objects. It has three parts:

- An entity represents one object. It is a plain ID with no data of its own.
- A component stores one piece of the data of an entity, such as a position or a health value.
- A system is a function that updates components on every step of the simulation.

An ECS is structure, meaning code that other code builds on. In Sarx, the simulation, the central AI
and the physics engine would all build on it.

Sarx also exists to teach how a game engine works. The developer builds every layer below the
gameplay by hand to learn it.

Does Sarx organise its objects with an ECS? If it does, does the developer write that ECS or take on
a library?

## Decision Drivers

- The developer builds every layer below the gameplay by hand, the entity system included, to learn
  how each one works.
- A library comes into Sarx only where it leaves the design of Sarx alone. It also has to hide
  nothing that the developer wants to learn and control. The section of the README on libraries sets
  out these questions.
- The physics engine runs on the ECS. Its data, such as velocity and mass, is stored in components.
  Each step of the physics is a system.
- The ECS is structure that other code builds on. Its first version follows the design that Sarx
  keeps. Later work extends that design rather than replacing it.

## Considered Options

- A hand-built ECS
- bevy_ecs
- hecs
- specs
- Plain structs, one per kind of object

## Decision Outcome

Chosen option: "A hand-built ECS", because building the ECS is part of what Sarx sets out to teach,
and each ECS library brings its own architecture and hides the part that the developer wants to
learn.

The developer writes the ECS in the `sarx-ecs` crate. Its first version moves the triangle from a
fixed-step simulation. That version is built on the design that Sarx keeps, and later work adds to
that design rather than replacing it.

How the ECS stores its data is a separate decision. ADR 0011 records it.

### Consequences

- Good, because the developer learns how entities are created and destroyed. They also learn how
  components are stored, and how systems find the components that they update.
- Good, because the ECS follows the design of Sarx itself. The simulation and the physics engine
  therefore stay free of the conventions of any library.
- Good, because an upgrade to a library can never change the API that the simulation builds on.
- Bad, because the developer writes, tests and fixes features that a library would provide
  ready-made. Queries over several components, and running systems in parallel, are two such
  features.
- Bad, because a mistake in the ECS reaches every system that builds on it.

### Confirmation

`cargo tree --workspace` lists no ECS library, such as bevy_ecs, hecs or specs.

## Pros and Cons of the Options

### A Hand-Built ECS

The developer writes the entities, the components, the systems and the code that ties them together.

- Good, because every part of the ECS is code in Sarx. The developer can read it, change it and step
  through it in a debugger.
- Good, because the ECS grows with the features that the game needs, one at a time.
- Bad, because every feature takes the developer's time before the simulation can use it.

### bevy_ecs

bevy_ecs is the ECS of the Bevy engine. It is published as a crate that works without the rest of
Bevy.

- Good, because it is widely used and actively maintained.
- Good, because its scheduler runs systems in parallel where their data allows it.
- Bad, because it brings its own architecture. Systems take their data through parameter types that
  Bevy defines. A `Schedule` decides when each system runs.
- Bad, because it hides the ECS, which is the part that the developer wants to learn.
- Bad, because its API changes with the releases of Bevy. Those releases often break existing code.

### hecs

hecs is a small ECS library. It provides a `World` of entities, and queries over their components.

- Good, because its API is small, and it leaves scheduling to the caller. Systems stay ordinary
  functions, which Sarx calls in its own order.
- Bad, because it hides how entities are allocated, how components are stored and how a query finds
  them.

### specs

specs is an older ECS library for Rust. It is best known as the ECS of the Amethyst engine.

- Good, because it has a long record of use in Rust games.
- Bad, because it brings its own architecture. Systems implement its `System` trait, and declare
  their data through its `SystemData` types. Its dispatcher then runs them.
- Bad, because it hides the ECS, which is the part that the developer wants to learn.
- Bad, because Amethyst ended development in 2022, as far as the author knows. specs appears to
  have seen little new work since.

### Plain Structs, One per Kind of Object

Sarx would define one struct for each kind of object, such as `Villager` and `Tree`. Each struct
would have the fields that its kind needs.

- Good, because it is the simplest design. The compiler checks every field of every type.
- Bad, because several kinds of object share some data and behaviour, such as a position and
  gravity. Plain structs write that code once for each kind, or route it through traits.
- Bad, because a new kind of object needs its own struct. It also needs a change to every function
  that walks the objects.
- Bad, because the physics engine needs an ECS. It stores its data in components and runs each of
  its steps as a system, as ADR 0010 records.

## More Information

The developer recorded this decision in the design decisions of the README on 17 October 2026. That
was the day on which the Sarx workspace was first set up.
