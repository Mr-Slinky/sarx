# Sarx

Sarx is a god game for phones and desktops, written in Rust on top of wgpu. The player guides a
civilisation on a flat world, trains a creature, and wins or loses ground against rival factions. It
draws on Black & White 2 for the creature, the god's hand and the choice between good and evil, and on
Universim and Spore for a civilisation that advances on its own.

The project has a second purpose alongside the game. Every layer below the gameplay, from the
renderer to the entity system, is built by hand so that its author learns how each one works.

## Planned Features

The author plans the following features. Each one is a goal, and the code has yet to implement any of
them.

- **Trainable creatures.** The player teaches a creature through reward and punishment, as in Black &
  White 2, and the creature's behaviour leans towards good or evil as a result. Its AI is meant to go
  well beyond what Black & White 2 offered.
- **A civilisation that improves itself.** Villages advance through new technologies and stages
  without the player directing each step, in the manner of Universim and Spore.
- **Rival factions.** Other factions compete for the same world. A faction can become an ally as well
  as an enemy.
- **Victory.** A player wins a level once their faction is the last one standing, either by allying
  with every other faction or by conquering them all. Factions are meant to respond to the path the
  player takes. They might ally with one another against a conqueror, or turn hostile over whom the
  player has allied with. Either path then grows harder the further the player takes it.
- **Levels.** A player either configures a level, choosing settings such as the number of factions
  and the kind of landscape, or picks a pre-made level.
- **Possession.** The player takes direct control of a single character to turn a battle, as Rise &
  Fall: Civilizations at War lets the player command a hero such as Alexander the Great. Possession
  of the creature comes first, and is very likely to be the only form. Possessing villagers or
  soldiers remains an option for later.
- **Abilities.** The player casts miracles and spells, drawing on the magic of both Universim and
  Black & White 2.
- **Cozy play.** Farming, fishing and other gentle activities let a player spend a session tending
  the world rather than fighting over it.
- **Cozy mode.** A player can switch on a mode in which no faction attacks them and they attack no
  one, leaving them to manage their civilisation in peace. A player in cozy mode cannot complete the
  level. A hostile action, a trade or any approach to a possible ally switches cozy mode off. The
  design also has to stop a player sheltering in cozy mode until their civilisation is large, and
  two approaches remain open:
  - The player chooses cozy mode before a level starts, and it stays fixed for that level.
  - Cozy mode allows only the actions that keep the civilisation going, without new buildings or
    population growth.

## Design Decisions

The author has taken the following decisions, and the code follows them as it grows.

- **A flat world.** The land is a flat heightmap, meaning a grid of terrain heights, as in Black &
  White 2.
- **Mobile first.** The renderer requests only the GPU limits that phones support. A desktop build
  then runs within those same limits.
- **Simulation apart from rendering.** The simulation advances in fixed steps, at the same rate on
  every device. The renderer draws the latest state as often as the screen allows. As a result, a
  phone that slows down when it gets hot plays the same game as a fast desktop.
- **Central AI.** Planners decide at the level of a group, such as a village assigning six people to
  chop wood. Each villager then carries out a simple task. This means the cost of thinking grows with
  the number of decisions, rather than with the number of villagers.
- **One gate for every change.** Every change to the world arrives as a request, a small piece of
  data stating which actor wants to do what. The player and the AI alike issue requests. A single
  gate checks each request against the rules of the level before it takes effect. As a result, a
  rule such as cozy mode becomes one check at the gate.
- **One level format.** The game loads every level from data in a single format, whatever produced
  it. A generator produces a level from the player's settings and a seed, and the same settings and
  seed always give the same level. A separate tool builds a level by hand in that same format, and
  can also refine a generated one. The game therefore loads a pre-made level and a configured level
  through the same code.
- **A hand-built ECS.** The entity component system is written for Sarx.
- **Libraries by decision.** The author weighs each library on its own before Sarx takes it on, and
  records the outcome as an Architecture Decision Record (ADR) in `docs/adr/`.

## Workspace

Sarx is a Cargo workspace, and each crate lives under `crates/`.

| Crate         | Contents                                                       |
|---------------|----------------------------------------------------------------|
| `sarx-ecs`    | The entity component system                                    |
| `sarx-sim`    | The game simulation, which depends on no graphics code         |
| `sarx-render` | The wgpu renderer                                              |
| `sarx-app`    | The program that opens the window and runs the other crates    |

New crates join the workspace as the project needs them.

## Building

The `rust-toolchain.toml` file selects the stable Rust toolchain, together with rustfmt, clippy and
the Android target. Rustup installs these automatically the first time Cargo runs inside the
repository.

On Windows, the toolchain is the MSVC one, which links with Microsoft's tools. A Windows machine
therefore needs the Visual Studio Build Tools, with the "Desktop development with C++" workload,
before its first build.

A developer then builds and runs the game from the repository root:

```
cargo run -p sarx-app
```

## TODO

The work starts with a thin path through every layer, ending in something on screen. Each layer then
grows on its own.

### Foundation

- [ ] Open a window and run the event loop
- [ ] Clear the screen, then draw a triangle
- [ ] Run the triangle on an Android phone
- [ ] Move the triangle from a fixed-step simulation with a minimal ECS
- [ ] Render a 3D cube with a camera and a depth buffer
- [ ] Render lit heightmap terrain with a touch and mouse camera

### Afterwards

The order of these follows from what the foundation teaches.

- [ ] Renderer: textures, shadows, water, instancing
- [ ] ECS: extend the minimal version into the full one
- [ ] Simulation: villagers, needs, the central AI planner
- [ ] The hand: picking points on the terrain, grabbing and throwing
- [ ] Tools: debug overlay, timing, GPU capture

## Branches

Development happens on the `dev` branch. Each finished stage of work is squash-merged into `main`,
meaning `main` gains one commit per stage.
