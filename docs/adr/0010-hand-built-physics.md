---
status: accepted
date: 2026-10-18
decision-makers: Kheagen Haskins
---

# A Hand-Built Physics Engine

## Context and Problem Statement

Sarx needs physics. A physics engine moves objects under forces such as gravity. It also detects
collisions between objects, and resolves them.

The god's hand shows why. The TODO in the README lists the hand, which grabs objects and throws them
across the terrain. A thrown object then has to fall under gravity. It comes to rest against the
ground or against other objects.

Sarx represents the objects in its world through its own entity component system (ECS), as ADR 0009
records. Each object is an entity. Its data is stored in components. Systems update those components
on every step of the simulation.

Sarx builds every layer below the gameplay by hand, to teach how each one works. Its conventions
keep game engines such as Bevy out. They weigh any other library against three questions: whether it
brings its own architecture, what it hides, and how much work it does.

Does Sarx write its own physics engine, or take on a physics library?

## Decision Drivers

- Moving objects under forces and resolving their collisions is a layer below the gameplay. It
  therefore falls within what Sarx sets out to teach.
- The ECS stores the data for every object in the world. Physics data stored anywhere else would be
  a second copy of the state of each object.
- A library that brings its own architecture commits Sarx to the design of its author.
- Sarx builds structure properly from its first version. Later work extends that structure rather
  than replacing it. The physics engine is structure, since the hand and other gameplay build on it.

## Considered Options

- A hand-built physics engine on Sarx's ECS
- Rapier
- Avian

## Decision Outcome

Chosen option: "A hand-built physics engine on Sarx's ECS", because physics is one of the layers
that Sarx exists to teach, and running it on the ECS keeps every object's state in one place.

The developer writes the physics engine. Physics data, such as velocity and mass, is stored in ECS
components. Each step of the physics is an ECS system. Applying gravity is one such step, and
resolving a collision is another.

### Consequences

- Good, because the developer writes and understands every step that moves an object or resolves a
  collision.
- Good, because physics data lives in the same components as the rest of the data of an object. Any
  other system reads the velocity of an object in the same way that it reads the position. The AI and
  the hand are two such systems.
- Good, because the physics engine does not add a library to the dependencies.
- Good, because each physics step is a separate system. The developer adds a new step, such as
  friction, as one more system. The existing steps stay as they are.
- Bad, because the developer builds and debugs the physics engine before any gameplay can depend on
  it. A library such as Rapier provides joints and handles fast-moving objects from the first day.
- Bad, because the physics engine depends on the ECS. Its design therefore has to fit the component
  storage that ADR 0011 settles.

### Confirmation

`cargo tree --workspace` does not list a physics crate, such as `rapier3d` or `avian3d`. Every
physics system reads and writes the ECS components that Sarx defines.

## Pros and Cons of the Options

### A Hand-Built Physics Engine on Sarx's ECS

The developer writes the forces, the movement, the collision detection and the collision response.
Each of them is an ECS system that works on the components that Sarx defines.

- Good, because it teaches how a physics engine moves objects and separates them.
- Good, because the physics engine shares one copy of the state of each object with every other
  system.
- Good, because the developer extends it one step at a time, in the order that the game needs.
- Bad, because every feature is the developer's own work to design and test. That covers everything
  from resting contact to fast-moving objects.

### Rapier

Rapier is a physics engine written in Rust by Dimforge. It is published as the `rapier2d` and
`rapier3d` crates.

- Good, because it is mature and widely used in Rust. It provides joints, continuous collision
  detection and a character controller.
- Good, because it does a large amount of work, covering the whole physics engine.
- Bad, because it brings its own architecture. Rapier keeps its own world of rigid bodies and
  colliders, which code reaches through handles. Sarx would copy the position of each object into
  that world before every physics step. It would then copy the results back into the ECS.
- Bad, because it hides the movement and collision code. That code is the part that Sarx sets out
  to teach.

### Avian

Avian is a physics engine built for the ECS of Bevy. It is published as the `avian2d` and `avian3d`
crates.

- Good, because it stores physics data in ECS components, and runs each step as an ECS system. Sarx
  chooses the same design.
- Bad, because it depends on Bevy, which the conventions of the project keep out.
- Bad, because its components and systems belong to the ECS of Bevy. Sarx would have to adopt that
  ECS in place of its own.
- Bad, because it hides the movement and collision code, as Rapier does.

## More Information

ADR 0011 settles how the ECS stores its components.

Two questions remain open. The developer settles each one in its own discussion:

- the integration method, meaning the way that each step turns forces into new velocities and
  positions
- the collision algorithm, meaning the way that the physics engine finds the objects that touch and
  pushes them apart
