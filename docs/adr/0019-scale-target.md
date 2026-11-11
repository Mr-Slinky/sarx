---
status: accepted
date: 2026-11-11
decision-makers: Kheagen Haskins
---

# Scale Target

## Context and Problem Statement

Sarx aims to simulate an order of magnitude more villagers than other games of its kind. Universim,
one such game, slows down badly as its population grows.

Many later decisions are trade-offs, such as change detection in the ECS. A design that suits 5,000
villagers can fail at 500,000. Without a target, each such decision would rest on its own guess at
the scale.

Scale matters most for memory. A system that updates every villager reads the components of each
villager once per step. The amount that it reads is the number of villagers times the size of the
components that it reads.

The time that a pass takes depends on where those bytes are stored. A CPU keeps recently used data
in caches, which are small memories on the chip. On a mid-level laptop, the L2 cache has room for
about 1 to 2 MB for each core, and the L3 cache, which all the cores share, for about 8 to 24 MB.
Data that does not fit in the caches comes from main memory. One thread reads main memory at roughly
10 to 20 GB per second.

No villager exists yet. As an estimate, a villager takes about 200 bytes of components: a position
and velocity, health and needs, AI state, and the pose of its rigid parts. The table below uses that
estimate. For each population, it gives the size of the data, where that data fits, and the time
that one pass over every villager takes.

| Villagers | Data    | Where it fits                 | One full pass     |
|-----------|---------|-------------------------------|-------------------|
| 5,000     | 1 MB    | the caches                    | under 0.1 ms      |
| 50,000    | 10 MB   | about the size of L3          | about 0.5 to 1 ms |
| 500,000   | 100 MB  | main memory                   | about 5 to 10 ms  |

ADR 0005 proposes a rate of 60 steps per second. At that rate, each step lasts 16.7 ms.

How many entities must Sarx handle, on what hardware?

## Decision Drivers

- Sarx aims to simulate an order of magnitude more villagers than games like it.
- ADR 0003 sets the target hardware as a mid-level laptop.
- Each step lasts 16.7 ms, at the rate that ADR 0005 proposes.
- The number of villagers on screen at once sets the work of the renderer, rather than that of the
  simulation.
- Trees, rocks and other static things far outnumber villagers on a large map. The systems that
  update villagers never visit them.

## Considered Options

- 5,000 villagers
- 50,000 villagers
- 500,000 villagers

## Decision Outcome

Chosen option: "50,000 villagers", because it is an order of magnitude beyond games like Sarx, and a
step on a mid-level laptop still has room for about a dozen full passes over every villager.

Sarx sets three limits:

- A world contains at most 50,000 villagers. This number sets the time that each step takes. The
  systems that run every step, such as movement, needs and animation, all update villagers.
- At most 15,000 villagers appear on screen at once, as during a large battle or with the whole city
  in view. This number sets what the renderer draws in one frame.
- A world contains at most 500,000 entities, villagers included. Most of the rest are static, such
  as trees and rocks. A query visits only the tables that store every component type that it asks
  for. A static entity therefore takes memory, but the systems that update villagers never visit it.

The limit of 500,000 entities is the figure for planning memory. It is also the figure for planning
the systems that cover the whole world, such as a spatial index. The limit of 50,000 villagers is
the one that every design must meet.

Particle effects, such as rain, smoke and sparks, must stay outside the ECS. Each one lasts only a
second or two, many thousands exist at once, and none of them affects the game. As an entity, each
one would need a table operation when it spawns, and another when it despawns. The renderer must
store them instead, in an array of its own or on the GPU. The simulation must start an effect by
sending an event, such as "a storm starts here". This follows ADR 0005, which keeps the simulation
apart from the renderer.

The test that separates an entity from a particle effect is whether the thing affects the game. An
arrow that can hit a villager is an entity. The trail behind the arrow is a particle effect.

### Consequences

- Good, because every later trade-off has one target to weigh against.
- Good, because a step that updates 50,000 villagers finishes with time to spare. Sarx can give that
  time to each villager, as a richer AI, more needs and more animation.
- Good, because a storm or a battle leaves the entity count unchanged, however many particle effects
  it shows.
- Bad, because the components of 50,000 villagers are already too large to stay in the caches for a
  full pass. Each system must read only the columns that it needs, and must avoid passes over every
  villager where it can.
- Bad, because the 200-byte estimate is a guess. A villager twice that size would halve the number
  of full passes that fit in a step.
- Bad, because the limit restricts the game. Before Sarx takes on a design that needs more than
  50,000 villagers, the developer must revisit this decision.

### Confirmation

Nothing in the code embodies this decision yet. Once villagers exist, a benchmark must confirm it.
The benchmark must spawn 50,000 villagers with their real components, and 450,000 static entities.
A step of the simulation must then finish within 16.7 ms on a mid-level laptop.

A reader also checks that no type for a particle effect implements `Component`.

## Pros and Cons of the Options

### 5,000 Villagers

- Good, because every villager fits in the caches, and the layout of components in memory barely
  matters.
- Bad, because it stays in the range of games like Sarx, rather than beyond it.

### 50,000 Villagers

- Good, because it reaches an order of magnitude beyond games like Sarx.
- Good, because a step still has room for about a dozen full passes.
- Bad, because each system must take care over which columns it reads.

### 500,000 Villagers

- Good, because it would set Sarx far apart from every game like it.
- Bad, because a step would have room for one or two full passes, at most.
- Bad, because the AI could not plan for every villager on every step. It would need to update most
  villagers rarely.
- Bad, because the ECS would need tables split into fixed-size chunks. That would change ADR 0011.
