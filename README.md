# Sarx

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![wgpu](https://img.shields.io/badge/wgpu-30.0.1-blue.svg)](https://wgpu.rs/)
[![winit](https://img.shields.io/badge/winit-0.30.13-blue.svg)](https://github.com/rust-windowing/winit)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://www.microsoft.com/windows)
![License](https://img.shields.io/badge/License-Proprietary-red.svg)

Sarx is a god game for Windows, written in Rust. It draws its graphics through wgpu, a Rust
graphics library.

During a level, the player does three things. They guide a civilisation on a flat world, they train
a creature, and they compete with rival factions for the land.

Three older games shape the design. Black & White 2 supplies the creature, the god's hand and the
choice between good and evil. Universim and Spore supply a civilisation that advances on its own.

The project also has a second purpose. The author builds every layer below the gameplay by hand, to
learn how each one works. Those layers run from the renderer to the entity system.

## Planned Features

The features below describe the game that the author is working towards. Each one is still a plan.

- The player trains a **creature**, teaching it by rewarding and punishing it, as in Black & White 2.
  Over time, the creature's behaviour leans towards good or evil. The author means its AI to go well
  beyond what Black & White 2 offered.
- The player's **civilisation** improves itself. Villages advance on their own, as they do in
  Universim and Spore. Each village moves through new technologies and stages without the player
  directing every step.
- **Rival factions** compete with the player for the same world. A faction can become an ally as
  well as an enemy.
- A level ends in **victory** for a player once their faction is the last one standing. There are
  two ways to reach that point. The player can ally with every other faction, or conquer them all.

  The factions are meant to respond to the path that the player takes. They might ally with one
  another against a conqueror. They might also turn hostile over whom the player has allied with.
  Either way, each path grows harder the further the player takes it.
- A player starts a **level** in one of two ways. They can configure a level, choosing
  settings such as the number of factions and the kind of landscape. Alternatively, they can pick a
  pre-made level.
- During a battle, the player can take direct control of a single character, through
  **possession**. Rise &
  Fall: Civilizations at War works the same way, letting the player command a hero such as Alexander
  the Great. The creature is the first character that the player can possess, and very likely the
  only one. Possessing villagers or soldiers remains an option for later.
- The player casts **miracles and spells**. Their magic draws on both Universim and Black & White 2.
- The world offers **cosy play**, gentle activities such as farming and fishing. A player can spend a
  whole session on these, tending the world.
- A player can switch on **cosy mode**. In cosy mode, no faction attacks the player, and the player
  does not attack anyone. The player is then free to manage their civilisation in peace.

  A player in cosy mode cannot complete the level. Three things switch cosy mode off: a hostile
  action, a trade, and any approach to a possible ally.

  The author also has to stop a player from exploiting cosy mode. A player could, for example, stay
  in cosy mode until their civilisation is large. Two approaches remain open:
  - The player chooses cosy mode before a level starts, and it stays fixed for that level.
  - Cosy mode allows only the actions that keep the civilisation going. It does not allow new
    buildings or population growth.

## Design Decisions

The author writes down each decision about a library or about the design. Each one goes in a short
document of its own, called an Architecture Decision Record (ADR). An ADR states what the author
decided, and why.

Every ADR is stored in `docs/adr/`. The table below lists them all, with a one-line summary of each.
The format of an ADR is set out further down, under Conventions.

| ADR                                                                  | Decision                        | In short                                                                                            |
|----------------------------------------------------------------------|---------------------------------|-----------------------------------------------------------------------------------------------------|
| [0001](docs/adr/0001-window-library.md)                              | winit as the window library     | Sarx opens its window and receives its events through winit                                         |
| [0002](docs/adr/0002-graphics-library.md)                            | wgpu as the graphics library    | Sarx draws through wgpu, which runs on DirectX 12, Vulkan, Metal and OpenGL                         |
| [0003](docs/adr/0003-windows-first.md)                               | Windows first                   | Sarx is built and tested on Windows, within GPU limits that a mid-level laptop meets                |
| [0004](docs/adr/0004-flat-world.md)                                  | A flat world                    | The land is a flat heightmap, a grid of terrain heights, as in Black & White 2                      |
| [0005](docs/adr/0005-simulation-apart-from-rendering.md)             | Simulation apart from rendering | The simulation advances in fixed steps on every device, and the renderer draws the latest state     |
| [0006](docs/adr/0006-central-ai.md)                                  | Central AI                      | A planner decides for a whole group, such as a village, and each villager carries out a simple task |
| [0007](docs/adr/0007-one-gate-for-every-change.md)                   | One gate for every change       | Every change to the world arrives as a request, which one gate checks against the level's rules     |
| [0008](docs/adr/0008-one-level-format.md)                            | One level format                | The game loads generated and hand-built levels from data in one format, through the same code       |
| [0009](docs/adr/0009-hand-built-ecs.md)                              | A hand-built ECS                | The author writes the entity component system, rather than taking on an ECS library                 |
| [0010](docs/adr/0010-hand-built-physics.md)                          | A hand-built physics engine     | The author writes the physics engine, which runs on the ECS                                         |
| [0011](docs/adr/0011-archetype-component-storage.md)                 | Archetype component storage     | The ECS stores each entity in a table for its exact set of components, one column per component     |
| [0012](docs/adr/0012-stylised-art.md)                                | Stylised art                    | Sarx looks stylised, as Astro Bot does, and builds each villager from separate rigid parts          |
| [0013](docs/adr/0013-miri-for-unsafe-code.md)                        | Miri for unsafe code            | Tests for `unsafe` code also run under Miri, a nightly-only tool that detects undefined behaviour   |
| [0014](docs/adr/0014-hand-poses-as-target-keyframes.md)              | Hand poses as target keyframes  | Each hand pose is a one-keyframe glTF animation, and each bone turns towards it by slerp            |
| [0015](docs/adr/0015-opt-in-component-trait.md)                      | Opt-in component trait          | Each component type must implement `Component` by hand, and the compiler rejects every other type   |
| [0016](docs/adr/0016-hand-written-gltf-reader.md)                    | Hand-written glTF reader        | The author writes the glTF reader, and `serde_json` turns its JSON text into Rust structs           |

## Workspace

The code is split into several crates. Together, the crates form one Cargo workspace.

Each crate has its own folder under `crates/`. The root `Cargo.toml` lists `crates/*` as the
workspace's members, a pattern that matches every folder there. As a result, a crate that a
developer creates under `crates/` with `cargo new` joins the workspace with no further setup.

The workspace contains four crates today:

| Crate       | Purpose                                                         |
|-------------|-----------------------------------------------------------------|
| `enki`      | The entity component system                                     |
| `zurvan`    | The game simulation, which does not depend on any graphics code |
| `amaterasu` | The wgpu renderer                                               |
| `atum`      | The program that opens the window and runs the game             |

The author adds crates as the project needs them.

### Crate Names

Each crate takes the name of a god whose domain matches the job of the crate. The god can come from
any pantheon.

- **Enki** was the Mesopotamian god of wisdom and crafts. He kept the *me*, the decrees that defined
  each art and institution of civilisation. `enki` keeps the components that define each entity in
  the game.
- **Zurvan** was the Persian god of infinite time. `zurvan` moves the world forward through time,
  one fixed step after another.
- **Amaterasu** is the Japanese goddess of the sun. When she hid in a cave, the world went dark.
  `amaterasu` makes the world visible.
- **Atum** was the first of the Egyptian gods. He created himself, then brought the other gods into
  being. `atum` is the program that starts first, and brings up the rest of the game.

## Building

Rustup sets up the Rust toolchain by itself. The `rust-toolchain.toml` file at the root selects the
stable toolchain. It also asks for two components: rustfmt, which formats code, and clippy, which
lints it. Rustup should install all three the first time that Cargo runs inside the repository.

A Windows machine also needs a set of tools from Microsoft. On a standard Windows install, the
default host for rustup is MSVC. Rustup therefore picks the MSVC toolchain, which links with
Microsoft's tools. Before its first build, a Windows machine needs the Visual Studio Build Tools,
with the "Desktop development with C++" workload.

A developer runs every command below from the repository root.

| Task              | Command                               |
|-------------------|---------------------------------------|
| Build every crate | `cargo build`                         |
| Run the game      | `cargo run -p atum`                   |
| Test every crate  | `cargo test`                          |
| Check unsafe code | `cargo +nightly miri test -p enki`    |
| Build one crate   | `cargo build -p <crate>`              |
| Lint every crate  | `cargo clippy --workspace`            |
| Add a dependency  | `cargo add -p <crate> <dependency>`   |

The unsafe code check needs Miri, which runs on the nightly toolchain. A developer installs both once,
with `rustup toolchain install nightly --component miri`.

## Conventions

Everyone who works on the project follows the conventions below.

### Libraries

The author takes on each library as a deliberate choice.

Before adding a library, the author discusses it and weighs its pros and cons. An ADR then records
the outcome. The next section sets out how an ADR is written.

Game engines, such as Bevy, stay out. An engine does the very work that the project exists to teach.

For every other library, three questions decide whether it comes in:

- **Does it bring its own architecture?** Some libraries are a set of separate functions. A library
  of that kind leaves the design of the game as it is. glam, a library of vector and matrix maths, is
  one example. The way that glam computes a dot product has no bearing on the game.

  A library that brings its own architecture is different. It commits Sarx to the design that its
  own authors chose, as an ECS library would.
- **What does it hide?** Every library hides some part of the work. Where that part is one that the
  author wants to learn and control, the library stays out. An ECS library, for example, hides the
  ECS.

  wgpu passes this test. It hides the differences between Vulkan, Metal and DirectX. The pipeline,
  shaders, buffers and draw calls stay in code that the author writes.
- **How much work does it do?** Work that a library does in place of hand-written code counts in its
  favour. glam is again the example. Writing its maths by hand would delay every layer that depends
  on that maths.

### Decision Records

Every ADR uses the same format, [MADR](https://adr.github.io/madr/) version 4. The points below set out how that format
applies here.

- Each ADR is a file of its own in `docs/adr/`. The files are numbered in order, such as
  `0001-window-library.md`.
- Each ADR opens with a YAML block. The block records the status of the ADR, its date and the people
  who made the decision.
- The sections then follow in the order that MADR sets:
  1. the context and problem statement
  2. the decision drivers
  3. the options considered
  4. the decision outcome, with its consequences and its confirmation
  5. the pros and cons of each option
- Each new ADR adds a row to the table under Design Decisions.
- An ADR always records the current decision. Where a decision changes, the author rewrites its ADR
  in place and updates the date. Git keeps the earlier versions.

### Tests

Unit tests follow the standard Rust layout. Each file keeps its tests in a `#[cfg(test)] mod tests`
block at its end.

Every test also follows five conventions:

- The tests for a function give it three kinds of **data**. Standard data is ordinary input.
  Extreme data lies at the edges of what the function accepts. Abnormal data is input that the
  function must reject.
- Where the same assertion repeats across inputs, one test covers every input from a **table of
  cases**. That one test replaces a copy of the test for each input.

  The `rstest` crate provides tests of this kind, through `#[rstest]` and `#[case]`. Like any other
  library, though, it awaits a decision. Until then, a test loops over an array of cases. Its
  assertion message states which case failed.
- Each test follows **arrange, act, assert**. It sets up its data, then acts, then asserts. Every
  assertion comes at the end.
- A test **name** follows the pattern `{function}_with_{data}_{outcome}`, in snake case.
  `height_at_with_point_outside_map_returns_none` is one such name.
- A test that expects a **failure** checks that the failure happens, and leaves the wording
  of its message unchecked. A refactor or a fix can reword a message while the code stays correct. A
  test that checked the wording would then fail, and force a change to the test as well as the code.
  A test that expects a panic therefore takes a bare `#[should_panic]`, with no `expected` text. A
  test that expects an `Err` checks which error it got, and leaves the text that `Display` writes
  unchecked.

### Commits

Every commit message follows the Conventional Commits format, `<type>[scope]: <description>`. The
type is one of `feat`, `fix`, `docs`, `style`, `refactor`, `test` and `chore`.

### Branches

The author develops on the `dev` branch, which keeps the full history of the work. The `main` branch
is the stable changelog. Each commit on `main` describes one finished piece of work, such as a crate
or the decision records.

A commit on `main` contains the finished files from `dev`, without the history behind them. Each
commit on `main` therefore reads as an independent piece of work, rather than as a merge from `dev`.

#### Bringing main up to date

GitHub accepts a change to `main` only through a pull request. The pull request also has to pass two
checks, `cargo test` and `cargo miri test`. The workflow in `.github/workflows/enki.yml` runs both
checks on every pull request into `main`.

A developer brings `main` up to date in five steps, starting from a clean working tree.

1. The developer creates a branch from `main` for the update:

   ```bash
   git switch -c stage/<name> main
   ```

2. The developer makes one commit for each piece of work. Each commit copies the finished files from
   `dev`. Where the work replaces files on `main`, such as a crate under a new name, the same commit
   deletes the old files:

   ```bash
   git rm -r crates/<old-crate>
   git checkout dev -- crates/<new-crate>
   git commit
   ```

   The commits go in an order in which each one builds. A crate comes after every crate that it
   depends on. A workflow arrives in the same commit as the code that it tests.

   The author of every commit on `main` is the developer, even where the commit contains an agent's
   work.
3. The developer runs `cargo test --workspace` on the branch. The developer then pushes the branch to
   GitHub, and opens a pull request into `main`.
4. Once both checks pass, the developer merges the pull request. The merge method depends on how
   many pieces of work the update brings in. One crate is one piece of work, as are the files in
   `docs/` and the files at the root of the repository.

   | Update                            | Merge method         | Result on `main`                       |
   |-----------------------------------|----------------------|----------------------------------------|
   | Brings in one piece of work       | **Squash and merge** | One commit, which shows Verified       |
   | Brings in several pieces of work  | **Rebase and merge** | One commit per piece of work, unsigned |

   An update with several pieces of work needs one commit for each piece on `main`. A squash merge
   would combine them into one, meaning only a rebase merge keeps them apart.

   A squash merge combines the commits on the branch into one commit. GitHub signs that commit with
   its own key, and the commit therefore shows a Verified badge. The developer gives the squash commit
   a message in the Conventional Commits format.

   A rebase merge writes a new copy of each commit onto `main`, keeping the commits apart. GitHub
   does not sign those copies, meaning the commits on `main` show no Verified badge, although the
   commits on the branch were signed.
5. The developer returns to `dev`, deletes the branch, and moves the local `main` to match GitHub:

   ```bash
   git switch dev
   git push origin --delete stage/<name>
   git branch -D stage/<name>
   git fetch --prune origin
   git branch -f main origin/main
   ```

   Both merge methods give the commits on `main` new IDs. The last command therefore points the local
   `main` at the commits on GitHub, in place of the commits on the branch.

## TODO

The work comes in two parts.

First, the author builds the simplest working version of every layer, from the window down to the
GPU. This first part ends with lit terrain on the screen. The Foundation list below covers it.

Second, the author extends each layer on its own. The Later Work list covers that part.

Improvements that come up along the way, and that no step below needs yet, go in
[docs/todo_deferred.md](docs/todo_deferred.md).

### Foundation

- [x] Open a window and run the event loop
- [x] Clear the screen
- [x] Draw a triangle
- [x] Rebuild the surface when the app suspends and resumes
- [x] Recreate the surface when wgpu reports it lost
- [ ] Move the triangle from a fixed-step simulation with a minimal ECS
- [ ] Render a 3D cube with a camera and a depth buffer
- [ ] Draw the god's hand and move it with the cursor across a flat ground plane
- [ ] Render lit heightmap terrain with a mouse and keyboard camera

Today, each vertex has one attribute, its position. A later step gives a vertex a second attribute,
such as a colour for the cube or a normal for the lit terrain. Two changes wait for that step:

- `Mesh` then stores a `Vertex` struct in place of `[f32; 3]`. That struct takes `#[repr(C)]`, for
  the following reason.

  The vertex buffer layout states a fixed byte offset for each attribute. Rust, however, lays out a
  plain struct in an order of its own choosing. `#[repr(C)]` keeps the fields in the order written,
  at offsets that can be worked out by hand.
- `Mesh` turns its vertices into bytes before they reach the GPU. The author weighs the `bytemuck`
  crate as the way to do that. It offers two things:
  - At compile time, its derive macro rejects any vertex type that has padding.
  - Its `cast_slice` function returns a `&[u8]` over the same memory as the vertex slice, without
    copying the vertices.

The hand comes before the terrain. Until the terrain exists, the hand moves across a flat plane at
height zero instead.

To place the hand on that plane, the game follows a ray. A ray is a line from the camera, through the
cursor, into the world. The hand goes to the point where the ray meets the plane.

Two questions wait for the hand step:

- The author decides how the mesh for the hand reaches the game. The game can build the mesh in code
  from simple shapes. Alternatively, it can load the mesh from a glTF file, a common format for 3D
  models. Loading glTF raises a choice between a library, the `gltf` crate, and a parser written by
  hand.
- Under ADR 0012, each villager is built from rigid parts. The author decides whether the hand is
  built the same way.

### Later Work

The items below come after the foundation. What the foundation teaches decides their order.

- [ ] Renderer: textures, shadows, water, instancing
  - Before writing any instancing code, the author works through three questions. How does one draw
    call repeat a mesh? How does a buffer with `VertexStepMode::Instance` advance one element per
    instance, rather than one per vertex? What does the instance range in `draw` control?
- [ ] ECS: extend the minimal version into the full one
- [ ] Simulation: villagers, needs, the central AI planner
- [ ] The hand: picking points on the terrain, grabbing and throwing, gestures
- [ ] Animation: villagers that move their rigid parts, with distant villagers updating less often;
  keyframes and procedural animation for the creature and the hand
  - A figure built from parts forms a hierarchy, in which the position of each part depends on the
    position of its parent. Before the ECS stores such a figure, the author settles how archetype
    storage keeps that hierarchy.
- [ ] Tools: debug overlay, timing, GPU capture
