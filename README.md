# Sarx

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![wgpu](https://img.shields.io/badge/wgpu-30.0.1-blue.svg)](https://wgpu.rs/)
[![winit](https://img.shields.io/badge/winit-0.30.13-blue.svg)](https://github.com/rust-windowing/winit)
![License](https://img.shields.io/badge/License-Proprietary-red.svg)

Sarx is a god game for phones and desktops, written in Rust on top of wgpu. The player guides a
civilisation on a flat world, trains a creature, and competes with rival factions for the land. It
draws on Black & White 2 for the creature, the god's hand and the choice between good and evil, and on
Universim and Spore for a civilisation that advances on its own.

The project has a second purpose alongside the game. The author builds every layer below the
gameplay by hand, from the renderer to the entity system, to learn how each one works.

## Planned Features

The author plans the following features. Each one describes the game that the author is working
towards.

- **Trainable creatures.** The player teaches a creature through reward and punishment, as in Black &
  White 2, and the creature's behaviour leans towards good or evil as a result. Its AI is meant to go
  well beyond what Black & White 2 offered.
- **A civilisation that improves itself.** Villages advance through new technologies and stages
  without the player directing each step, in the manner of Universim and Spore.
- **Rival factions.** Other factions compete for the same world. A faction can become an ally as well
  as an enemy.
- **Victory.** A player wins a level once their faction is the last one standing, either by allying
  with every other faction or by conquering them all. Factions are meant to respond to the path that
  the player takes. They might ally with one another against a conqueror, or turn hostile over whom the
  player has allied with. Either path then grows harder the further the player takes it.
- **Levels.** A player either configures a level, choosing settings such as the number of factions
  and the kind of landscape, or picks a pre-made level.
- **Possession.** The player takes direct control of a single character during a battle, as Rise &
  Fall: Civilizations at War lets the player command a hero such as Alexander the Great. Possession
  of the creature comes first, and is very likely to be the only form. Possessing villagers or
  soldiers remains an option for later.
- **Abilities.** The player casts miracles and spells, drawing on the magic of both Universim and
  Black & White 2.
- **Cosy play.** Farming, fishing and other gentle activities let a player spend a whole session
  tending the world.
- **Cosy mode.** A player can switch on a mode in which no faction attacks them and they attack no
  one, leaving them to manage their civilisation in peace. A player in cosy mode cannot complete the
  level. A hostile action, a trade or any approach to a possible ally switches cosy mode off. The
  design also has to stop a player from staying in cosy mode until their civilisation is large (or
  similar exploits). Two approaches remain open:
  - The player chooses cosy mode before a level starts, and it stays fixed for that level.
  - Cosy mode allows only the actions that keep the civilisation going, without new buildings or
    population growth.

## Design Decisions

The author has taken the following decisions and writes the code to follow them.

- **A flat world.** The land is a flat heightmap, meaning a grid of terrain heights, as in Black &
  White 2.
- **Mobile first.** The renderer requests only the GPU limits that phones support. A desktop build
  then runs within those same limits.
- **Simulation apart from rendering.** The simulation advances in fixed steps, at the same rate on
  every device. The renderer draws the latest state as often as the screen allows. As a result, a
  phone that slows down when it gets hot plays the same game as a fast desktop.
- **Central AI.** Planners decide at the level of a group, such as a village assigning six people to
  chop wood. Each villager then carries out a simple task. This means the planning work grows with the
  number of decisions, rather than with the number of villagers.
- **One gate for every change.** Every change to the world arrives as a request, a small piece of
  data stating which actor wants to do what. The player and the AI alike issue requests. A single
  gate checks each request against the rules of the level before it takes effect. As a result, a
  rule such as cosy mode becomes one check at the gate.
- **One level format.** The game loads every level from data in a single format, whatever produced
  it. A generator produces a level from the player's settings and a seed, meaning a number that fixes
  every random choice that the generator makes. The same settings and seed always give the same
  level. With a separate tool, a person builds a level by hand in that same format, or refines a
  generated one. The game therefore loads a pre-made level and a configured level through the same
  code.
- **A hand-built ECS.** The author writes the entity component system (ECS) for Sarx. An ECS
  represents each object in the game, such as a villager or a tree, as a plain ID called an entity.
  The data for each entity is stored in components, such as a position or a health value. Systems are
  the functions that update those components on every step of the simulation.
- **A hand-built physics engine.** The author also writes the physics engine for Sarx. A physics
  engine moves objects under forces such as gravity, and detects and resolves collisions between
  them. Sarx's physics engine runs on the ECS. Physics data, such as velocity and mass, is stored in
  components, and each step of the physics, such as applying gravity or resolving a collision, is a
  system.

## Workspace

Sarx is a Cargo workspace. Each crate has its own folder under `crates/`, and the root `Cargo.toml`
lists `crates/*` as its members. A new folder there therefore joins the workspace without further
setup.

| Crate         | Contents                                                     |
|---------------|--------------------------------------------------------------|
| `sarx-ecs`    | The entity component system                                  |
| `sarx-sim`    | The game simulation, which depends on no graphics code       |
| `sarx-render` | The wgpu renderer                                            |
| `sarx-app`    | The program that opens the window and calls the other crates |

The author adds crates as the project needs them.

## Building

The `rust-toolchain.toml` file selects the stable Rust toolchain, together with rustfmt, clippy and
the `aarch64-linux-android` target. Rustup installs these automatically the first time Cargo runs
inside the repository.

On Windows, the toolchain is the MSVC one, which links with Microsoft's tools. A Windows machine
therefore needs the Visual Studio Build Tools, with the "Desktop development with C++" workload,
before its first build.

A developer runs every command below from the repository root.

| Task              | Command                             |
|-------------------|-------------------------------------|
| Build every crate | `cargo build`                       |
| Run the game      | `cargo run -p sarx-app`             |
| Test every crate  | `cargo test`                        |
| Build one crate   | `cargo build -p <crate>`            |
| Lint every crate  | `cargo clippy --workspace`          |
| Add a dependency  | `cargo add -p <crate> <dependency>` |

## Conventions

Everyone who works on Sarx follows the conventions below.

### Libraries

Sarx takes on a library only after a discussion that weighs its pros and cons. An Architecture
Decision Record then records the outcome, as the next section sets out. Game engines such as Bevy
stay out, since an engine does the work that Sarx exists to teach. Three questions decide whether
any other library comes in:

- **Does it bring its own architecture?** A library of separate functions leaves Sarx's design
  alone. glam is the model, since the way it computes a dot product has no bearing on the game. A
  library that brings its own architecture commits Sarx to its author's design, as an ECS library
  would.
- **What does it hide?** A library that hides a part that the author wants to learn and control
  stays out. An ECS library hides the ECS, for example. wgpu hides the differences between Vulkan,
  Metal and DirectX, and leaves the pipeline, shaders, buffers and draw calls to Sarx's own code.
- **How much work does it do?** The work that a library does in place of Sarx's own code counts in
  its favour. Writing the maths that glam provides by hand would delay every layer that depends on
  it.

### Decision Records

An Architecture Decision Record (ADR) is a short document that states what the author decided and
why. Each decision on a library or on the design gets one. Sarx writes its ADRs in the
[MADR](https://adr.github.io/madr/) format, version 4.

- ADRs go in `docs/adr/`, one file per decision, numbered in order, such as
  `0001-window-library.md`.
- Each ADR opens with a YAML block that records its status, its date and the people who made the
  decision.
- The sections then follow in MADR's order: the context and problem statement, the decision drivers,
  the options considered, the decision outcome with its consequences, and the pros and cons of each
  option.
- An accepted ADR stays as written. Where a decision changes, a new ADR records the change. The
  author then sets the old ADR's status to `superseded by ADR-NNNN`, with the new ADR's number in
  place of `NNNN`.

### Tests

Unit tests go in a `#[cfg(test)] mod tests` block at the end of the file that they test, as is
standard in Rust. Every test follows four conventions:

- **Data.** The tests for a function give it standard data, extreme data at the edges of what it
  accepts, and abnormal data that it must reject.
- **Cases in a table.** Where the same assertion repeats across inputs, one test covers every input,
  in place of a copy of the test for each. The `rstest` crate provides parameterised tests through
  `#[rstest]` and `#[case]`. Like any other library, it awaits a decision. Until then, a test
  loops over an array of cases, with an assertion message that states the case that failed.
- **Arrange, act, assert.** Each test sets up its data, acts, then asserts, with every assertion at
  the end.
- **Naming.** A test name follows the pattern `{function}_with_{data}_{outcome}` in snake case, such
  as `height_at_with_point_outside_map_returns_none`.

### Commits

Commit messages follow the Conventional Commits format, `<type>[scope]: <description>`. The types
are `feat`, `fix`, `docs`, `style`, `refactor`, `test` and `chore`.

### Branches

The author develops on the `dev` branch and squash-merges each finished stage into `main`, meaning
`main` gains one commit per stage.

## TODO

The author first builds the simplest working version of every layer, from the window down to the
GPU, ending in lit terrain on screen. The author then extends each layer on its own.

### Foundation

- [x] Open a window and run the event loop
- [x] Clear the screen
- [x] Draw a triangle
- [ ] Run the triangle on an Android phone
- [ ] Move the triangle from a fixed-step simulation with a minimal ECS
- [ ] Render a 3D cube with a camera and a depth buffer
- [ ] Render lit heightmap terrain with a touch and mouse camera

Two changes wait for the first step that gives a vertex a second attribute, such as a colour for the
cube or a normal for the lit terrain:

- `Mesh` stores a `Vertex` struct in place of `[f32; 3]`, and `Vertex` takes `#[repr(C)]`. Rust lays
  out a plain struct in an order of its own choosing, while the vertex buffer layout states a fixed
  byte offset for each attribute. `#[repr(C)]` keeps the fields in the order written, at offsets that
  can be worked out by hand.
- The author weighs `bytemuck` as the way that `Mesh` turns its vertices into bytes. Its derive macro
  rejects a vertex type that has padding while the code compiles, and its `cast_slice` reads the
  vertices as bytes without copying them.

### Later Work

What the foundation teaches decides the order of these items.

- [ ] Renderer: textures, shadows, water, instancing
  - Instancing opens with a full explanation before any code: how one draw call repeats a mesh, how
    `VertexStepMode::Instance` moves a second buffer on once per copy, and what the instance range
    in `draw` controls.
- [ ] ECS: extend the minimal version into the full one
- [ ] Simulation: villagers, needs, the central AI planner
- [ ] The hand: picking points on the terrain, grabbing and throwing
- [ ] Tools: debug overlay, timing, GPU capture
