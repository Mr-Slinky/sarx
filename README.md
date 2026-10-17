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
- **Possession.** The player takes direct control of a single character to turn a battle, much as
  some strategy games let the player command a hero such as Alexander the Great. The first version
  lets the player possess the creature.
- **Abilities.** The player casts miracles and spells, drawing on the magic of both Universim and
  Black & White 2.
- **Cozy play.** Farming, fishing and other gentle activities let a player spend a session tending
  the world rather than fighting over it.

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
- **A hand-built ECS.** The entity component system is written for Sarx. Libraries come in where they
  provide a broad toolkit, such as glam for vectors and matrices.

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

## Branches

Development happens on the `dev` branch. Each finished stage of work is squash-merged into `main`,
meaning `main` gains one commit per stage.
