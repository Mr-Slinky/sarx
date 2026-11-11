---
status: accepted
date: 2026-10-17
decision-makers: Kheagen Haskins
---

# A Flat World

## Context and Problem Statement

Sarx is a god game. The player guides a civilisation across the land. The player also reaches into
the world with the god's hand, and competes with rival factions for territory.

Every one of these activities takes place on the terrain. The terrain is the ground that villagers
walk on and that buildings stand on.

The shape of the world decides how the game stores the terrain, how the renderer draws it, and how
the hand finds the point on the ground below the cursor.

Sarx draws on three older games. It takes the creature and the god's hand from Black & White 2. It
takes a civilisation that advances on its own from Universim and Spore. Both Universim and Spore set
their civilisations on round planets.

What shape does the world take, and how does the game store its terrain?

## Decision Drivers

- Sarx follows Black & White 2 in the creature, the god's hand and the choice between good and evil.
  The world that these features play out on comes from the same game.
- The hand picks points on the terrain. It then grabs and throws what it finds there.
- A player can configure a level, choosing settings such as the kind of landscape. A generator then
  produces the level from those settings and from a seed. The seed is a number that fixes each
  random choice that the generator makes.
- The developer builds the terrain by hand, as with every other layer below the gameplay.

## Considered Options

- A flat heightmap
- A spherical planet
- Voxel terrain

## Decision Outcome

Chosen option: "A flat heightmap", because it gives Sarx the world of Black & White 2, the game that
Sarx follows for the creature and the god's hand.

The land is a flat heightmap. A heightmap is a grid of terrain heights, with one height stored at
each point of the grid. The grid lies flat. Each height lifts the ground at its point to that height.

### Consequences

- Good, because the terrain mesh follows directly from the grid. Each grid point becomes one vertex.
  Each square of four neighbouring points becomes two triangles.
- Good, because the height at any position on the land comes from the four grid points around it.
  The hand and the simulation can therefore place an object on the ground with a few arithmetic
  steps. A villager, a tree and a thrown rock are all placed in this way.
- Good, because a generator produces a level by computing one height for each grid point. It
  computes each height from the player's settings and the seed.
- Bad, because each grid point stores a single height. Caves, overhangs and arches need a second
  surface above or below the first. A heightmap has only the one surface.
- Bad, because the world ends at the edges of the grid. The camera, the creature and the villagers
  each need a rule for what happens there.

### Confirmation

Nothing in the code embodies this decision yet. Neither `amaterasu` nor `zurvan` contains any
terrain.

The foundation work in the README ends with lit heightmap terrain on screen. Once that step is done,
a reader can confirm the decision. They check that the terrain stores one height for each point of a
flat grid.

## Pros and Cons of the Options

### A Flat Heightmap

The world is a flat grid, with one terrain height stored at each grid point. Black & White 2 builds
its islands this way.

- Good, because the data is small. A grid of 1024 by 1024 points needs about 4 MiB, at one 32-bit
  float per height.
- Good, because the point on the ground below the cursor is quick to find. A ray is a line from the
  camera through the cursor into the world. The code steps along that ray across the regular grid,
  and tests the height at each step.
- Good, because the up direction is the same everywhere on the map. The camera, gravity and movement
  therefore all share one fixed direction.
- Bad, because the terrain has one surface, with no room for caves or overhangs.
- Bad, because the map has edges, which the game has to handle.

### A Spherical Planet

The world is a planet. The terrain rises and falls along the line from the centre of the planet out
to its surface. Universim and Spore set their civilisations on planets of this kind.

- Good, because the world has no edges. A player can travel all the way round and arrive back at the
  start.
- Neutral, because a planet still stores its terrain as heights. Each height runs along the line out
  from the centre, rather than straight up.
- Bad, because the up direction points away from the centre. It therefore differs at every point on
  the surface. The camera, gravity and every movement have to work it out for each position.
- Bad, because a flat grid cannot cover a sphere evenly. A grid wrapped round a sphere stretches its
  cells near the poles. As a result, a planet needs a different layout. One such layout is a cube
  with each face bent outwards into a sphere.

### Voxel Terrain

The world is a 3D grid of cells, called voxels. Each voxel is either solid ground or open space.
Minecraft builds its world this way.

- Good, because the terrain can contain caves, overhangs and arches.
- Good, because the player can dig into the ground and build on it, cell by cell.
- Bad, because the data grows with the volume of the world rather than its area. Take a world that
  is 1024 by 1024 cells across and 256 cells high. It has 256 times as many cells as a heightmap of
  the same width.
- Bad, because the cells have to become a smooth surface before the renderer can draw them. That
  step needs a meshing algorithm, such as marching cubes.
