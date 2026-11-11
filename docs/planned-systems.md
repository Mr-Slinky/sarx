# Planned Systems

A system is the code that updates one part of the game on each step of the simulation. This file
lists every system that the planned features of Sarx would need, and how each one would use the
tables of the ECS.

The list exists to guide design. The ECS is built for the systems that will run on it. Before the
developer or an agent designs a part of the ECS, or plans a feature, they read this list and check
the design against the systems that it must serve.

None of these systems exists yet. Every component name below, such as `Needs` or `Task`, is a
working guess, and the counts come from the limits that [ADR 0019](adr/0019-scale-target.md) sets:
50,000 villagers, 500,000 entities, and 15,000 villagers on screen at once.

## Keeping the List Current

A new planned feature adds its systems to the tables below. Where a feature changes, the rows for its
systems change with it. Each section opens with the sources that its systems come from, such as
the README, the TODO list, an ADR or the developer's own notes.

## Systems by Feature

### The God's Hand

The hand comes from the TODO list, [ADR 0014](adr/0014-hand-poses-as-target-keyframes.md) and
[ADR 0021](adr/0021-telekinetic-hand.md).

| System | Work on each step                                                                       | Reads and writes                                                                    | Entities                      |
|--------|-----------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|-------------------------------|
| Aim    | Casts a ray from the camera through the cursor, and finds the point where it meets the ground | reads the input, the camera and the heightmap; writes the target of the hand       | 1                             |
| Hover  | Raises the hand over whatever lies beneath it                                           | reads the collision shape of the hand and the positions of objects beneath it; writes the `Position` of the hand | 1, plus a few nearby objects |
| Pose   | Turns each bone of the hand towards the target pose                                     | writes the rotation of each bone                                                    | 1, with about 20 bones        |
| Lift   | Moves a held object towards the point beneath the hand. On release, the object keeps its velocity | writes the `Position` and `Velocity` of the held object                     | 1 to a few, by handle         |
| Strike | During a flick or a slap, pushes away whatever the hand meets                           | writes the `Velocity` of each object that the hand meets                            | a few, by handle              |

### Physics

Physics comes from [ADR 0010](adr/0010-hand-built-physics.md),
[ADR 0012](adr/0012-stylised-art.md) and [ADR 0021](adr/0021-telekinetic-hand.md).

| System        | Work on each step                                     | Reads and writes                                                              | Entities                                    |
|---------------|-------------------------------------------------------|-------------------------------------------------------------------------------|---------------------------------------------|
| Gravity       | Pulls each object in the air downwards                | writes `Velocity`                                                             | hundreds to thousands of thrown objects and loose parts |
| Integrate     | Adds the velocity of each object to its position      | reads `Velocity`; writes `Position`                                           | every moving object, up to 50,000           |
| Ground        | Keeps each object on or above the heightmap           | reads the heightmap; writes `Position`                                        | every moving object                         |
| Spatial index | Records the grid cell that each object lies in        | reads `Position`; writes the grid                                             | up to 500,000                               |
| Collisions    | Finds each pair of overlapping objects, and pushes the two apart | reads and writes `Position` and `Velocity` on two entities at once | sparse pairs, by handle                     |

### Villagers

Villagers come from [ADR 0006](adr/0006-central-ai.md), [ADR 0012](adr/0012-stylised-art.md) and
[ADR 0019](adr/0019-scale-target.md).

| System          | Work on each step                                                   | Reads and writes                                                           | Entities                                   |
|-----------------|---------------------------------------------------------------------|----------------------------------------------------------------------------|--------------------------------------------|
| Needs           | Lowers hunger, energy and every other need                          | writes `Needs`                                                             | 50,000                                     |
| Planner         | Decides for a whole village, such as sending six villagers to chop wood | reads the totals of the village; writes `Task` on the villagers that it chooses | one per village, with a few writes each |
| Tasks           | Carries out the task that the planner assigned                      | reads `Task` and `Position`; writes a target or `Velocity`                 | 50,000                                     |
| Steering        | Walks each villager towards its target                              | reads the target and `Position`; writes `Velocity`                         | 50,000                                     |
| Part animation  | Moves the head, body, hands and feet of each villager               | writes the transform of each part                                          | 50,000, with distant villagers updated less often |
| Part transforms | Places each part in the world, from the position of its villager    | reads the `Position` of the villager and the offset of each part; writes the world transform of each part | 50,000 villagers, with about 6 parts each |

### Combat

Combat comes from the Planned Features in the README, and from [ADR 0012](adr/0012-stylised-art.md).

| System    | Work on each step                                        | Reads and writes                                         | Entities                |
|-----------|----------------------------------------------------------|----------------------------------------------------------|-------------------------|
| Damage    | Applies each hit that the collisions report              | writes `Health`                                          | a few, by handle        |
| Dismember | Turns a head or limb that a blow knocks off into an object of its own | spawns an entity for the part, and removes the part from its villager | a few, in bursts |
| Death     | Removes each villager that has died                      | despawns                                                 | a few                   |

### The Creature

The creature comes from [ADR 0022](adr/0022-rigid-creature-that-places-its-feet.md) and the
developer's own notes.

| System      | Work on each step                                                                         | Reads and writes                          | Entities                  |
|-------------|-------------------------------------------------------------------------------------------|-------------------------------------------|---------------------------|
| Creature AI | Chooses where the creature goes, from its training. Under possession, the player chooses instead | writes the goal of the creature    | 1                         |
| Route       | Plans a path along the roads, and between the buildings                                   | reads the roads and the buildings         | 1                         |
| Feet        | Chooses where each foot lands next, then turns the joints of the leg until the foot reaches it | writes the rotation of each leg segment | 1, with a few legs     |

### Civilisation, Factions and Levels

These systems come from the Planned Features in the README,
[ADR 0007](adr/0007-one-gate-for-every-change.md) and [ADR 0008](adr/0008-one-level-format.md).

| System     | Work on each step                                                              | Reads and writes                                     | Entities                          |
|------------|--------------------------------------------------------------------------------|------------------------------------------------------|-----------------------------------|
| Progress   | Moves each village through new technologies and stages                         | reads and writes the state of each village           | a handful                         |
| Diplomacy  | Tracks allies, enemies and cosy mode                                           | reads and writes the state of each faction           | a handful                         |
| Gate       | Checks each request against the rules of the level, and applies each one that passes | writes any component, and makes structural changes | sparse                       |
| Growth     | Adds births and finished buildings, and grows crops                            | spawns; writes the growth of each crop               | sparse, plus thousands of crops   |
| Level load | Builds the whole level                                                         | spawns                                               | up to 500,000, once, in one burst |

### Handing the State to the Renderer

The renderer comes from [ADR 0005](adr/0005-simulation-apart-from-rendering.md),
[ADR 0019](adr/0019-scale-target.md) and [ADR 0020](adr/0020-change-detection.md).

| System  | Work on each frame                                   | Reads and writes     | Entities                  |
|---------|------------------------------------------------------|----------------------|---------------------------|
| Extract | Copies the transform of everything on screen         | reads transforms     | up to 15,000 villagers    |

## How Systems Use the Tables

Each system above uses the tables in one of four ways. A design for the ECS serves all four, and
puts the first one ahead of the others, since most of the time in a step goes there.

1. **A full pass over a few columns.** Needs, tasks, steering, integrate, part animation and extract
   each visit every row of every table that has their component types. Each one reads or writes
   only a few columns.
2. **A few entities, picked by handle.** The hand, the assignments of the planner, damage and
   collisions each reach a handful of entities through their handles. Collisions reach two entities
   in the same columns at once.
3. **Structural changes.** Spawning, despawning, adding a component and removing one are rare while
   a level runs, apart from bursts during combat. Loading a level spawns up to 500,000 entities in
   one burst.
4. **Single things and small groups.** The hand, the creature, each village and each faction is one
   entity, or one value that the whole game shares.

## Open Questions

The list raises questions that a design for the ECS must answer.

- **Villager parts.** A villager has about 6 rigid parts, under ADR 0012. Were each part an entity
  of its own, 50,000 villagers would need 300,000 entities for their parts alone. With the villagers
  themselves, that comes to 350,000 of the 500,000 entities that ADR 0019 allows, which leaves
  150,000 for every tree, rock, building and thrown object. The TODO list already asks how the
  tables store the hierarchy of a figure built from parts.
