# Sarx

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![wgpu](https://img.shields.io/badge/wgpu-30.0.1-blue.svg)](https://wgpu.rs/)
[![winit](https://img.shields.io/badge/winit-0.30.13-blue.svg)](https://github.com/rust-windowing/winit)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://www.microsoft.com/windows)
![License](https://img.shields.io/badge/License-Proprietary-red.svg)

Sarx is a god game, written in Rust, and made for Windows.

This game serves to fill a hole in the current landscape of the god game genre. Many, if not all,
contemporary god games are glorified resource management systems. Sarx aims to deliver the
entertaining sandbox experience that was found in earlier games of the genre, many years ago.

Sarx draws heavy inspiration from the Black & White series. Three features are the focus of the
first sprint of development:

1. The player controls a large, disembodied hand that interacts with the many features of the
   world.
2. A creature accompanies the player's followers, and the player trains it to behave in line with
   their play style.
3. The player shares the map with up to three rival factions, and wins by defeating every one of
   them. The player defeats each faction in one of two ways: by besieging it, or by building a city
   impressive enough that the faction's people immigrate to it. Each faction can fall a different
   way, so the player can besiege one faction and win over another. As a result, combat is CENTRAL
   to the game.

Inspiration aside, Sarx does not strive to be a clone or general copy, and has several plans for
innovation within the genre. It is the opinion of the developer that contemporary god games
(2020 onwards) severely miss the mark on what a god game is supposed to be. These games have made
poor choices in terms of which game mechanics to follow, and have left out the best and most fun
mechanic that a god game has to offer: the toybox!! Sarx features a creature much like the one in
the Black & White series, but implemented far more deliberately. Creatures are slower and more
powerful, and play an increasingly important role in winning the game.

Modern god games are also mostly about resource management, which makes very little sense for the
genre. Sarx features resources as a salient mechanic within the game, but NOT within a management
context. Rather, resources are tiered and directly related to the progress of a population. In
Sarx, villagers are made out of materials instead of flesh and bone. This makes the game title
ironic, while also opening the door for MANY unique game mechanics.

For example, the game can implement "gore" in relation to the material that makes up each villager.
A villager made of clay splatters. A villager made of glass shatters. A villager made of wood
splinters. And so on. War with neighbouring populations then means that the populations themselves
serve as resources. This also turns the art style into something achievable by a solo developer.

[docs/feature-list.md](docs/feature-list.md) lists every planned feature.

## Built by Hand

A large goal of the project is personal education. For this reason, a lot of the code is hand-built
despite the existence of an established library for that specific sub-problem. Much of this work
is already done by, for example, the Bevy engine. However, using these tools hides the underlying
concepts that the developer has not had the opportunity to learn.

It is very possible that hand-built sections are replaced with more established libraries in the
future. Such a replacement can happen once a module or crate has taught what it was built to teach,
and the effort for a more robust implementation of the same hand-built section outweighs the
educational purpose.

There is little reason to build a file-parsing library that can handle all nuances of parsing
.json files, only to spend a year on this tangential crate that does little to allow innovation.

## Workspace

The code is split into several crates, which together form one Cargo workspace.

Each crate has its own folder under `crates/`. The root `Cargo.toml` lists `crates/*` as the
workspace's members, a pattern that matches every folder there. As a result, a crate that a
developer creates under `crates/` with `cargo new` joins the workspace with no further setup.

The workspace contains five crates:

| Crate       | Purpose                                                         |
|-------------|-----------------------------------------------------------------|
| `enki`      | The entity component system                                     |
| `zurvan`    | The game simulation, which does not depend on any graphics code |
| `amaterasu` | The wgpu renderer                                               |
| `hermes`    | The glTF reader, which reads meshes from binary glTF files      |
| `atum`      | The program that opens the window and runs the game             |

Crates are added to the project as it needs them.

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

The `rust-toolchain.toml` file at the root selects the stable toolchain, along with two components:
rustfmt, which formats code, and clippy, which lints it. Rustup installs the toolchain and both
components the first time that Cargo runs inside the repository.

On a standard Windows install, rustup picks the MSVC toolchain, which links programs with
Microsoft's tools. Before its first build, a Windows machine therefore needs the Visual Studio Build
Tools, with the "Desktop development with C++" workload.

The developer can run the following commands from the repository root.

| Task              | Command                             |
|-------------------|-------------------------------------|
| Build every crate | `cargo build`                       |
| Run the game      | `cargo run -p atum`                 |
| Test every crate  | `cargo test`                        |
| Check unsafe code | `cargo +nightly miri test -p enki`  |
| Build one crate   | `cargo build -p <crate>`            |
| Lint every crate  | `cargo clippy --workspace`          |
| Add a dependency  | `cargo add -p <crate> <dependency>` |

The unsafe code check needs Miri, which runs on the nightly toolchain. The developer installs both
once, with `rustup toolchain install nightly --component miri`.

## Conventions

Everyone who works on Sarx follows the conventions in
[docs/repo-conventions.md](docs/repo-conventions.md). They cover libraries, decision records,
naming, tests, commits and branches.

## Decision Records

Each decision about a library or about the design is written down in an ADR. An ADR states what was
decided, and why.

An ADR records the best decision that the developer could make at the time, and argues for it in the
open so that anyone can challenge it. It is not fixed for good. How firmly a decision holds depends
on how much code is built on it, and each ADR therefore has one of two statuses:

- An **accepted** ADR is one that the code already acts on. Changing it means changing that code as
  well, and the cost grows as the decision spreads through the codebase.
- A **proposed** ADR is one that no code acts on yet, such as the central AI of ADR 0016. Changing it
  costs little. By the time the work reaches it, the developer will know more than when they wrote
  it, and many proposed ADRs will change before they are accepted.

The developer designs the code so that any decision costs as little as possible to change, although
that is easier said than done.

Every ADR is stored in `docs/adr/`. The table below lists them all, with a one-line summary of each.
The accepted ADRs come first, from 0001 to 0014, and the proposed ADRs follow. The
[Architectural Decisions](docs/repo-conventions.md#architectural-decisions) section of the
conventions sets out the format of an ADR, and how ADRs are numbered.

| ADR                                                          | Decision                        | In short                                                                                              |
|--------------------------------------------------------------|---------------------------------|-------------------------------------------------------------------------------------------------------|
| [0001](docs/adr/0001-window-library.md)                      | winit as the window library     | Sarx opens its window and receives its events through winit                                           |
| [0002](docs/adr/0002-graphics-library.md)                    | wgpu as the graphics library    | Sarx draws through wgpu, which runs on DirectX 12, Vulkan, Metal and OpenGL                           |
| [0003](docs/adr/0003-windows-first.md)                       | Windows first                   | Sarx is built and tested on Windows, within GPU limits that a mid-level laptop meets                  |
| [0004](docs/adr/0004-simulation-apart-from-rendering.md)     | Simulation apart from rendering | The simulation advances in fixed steps on every device, and the renderer draws the latest state       |
| [0005](docs/adr/0005-hand-built-ecs.md)                      | A hand-built ECS                | The developer writes the entity component system, rather than taking on an ECS library                |
| [0006](docs/adr/0006-hand-built-physics.md)                  | A hand-built physics engine     | The developer writes the physics engine, which runs on the ECS                                        |
| [0007](docs/adr/0007-archetype-component-storage.md)         | Archetype component storage     | The ECS stores each entity in a table for its exact set of components, one column per component       |
| [0008](docs/adr/0008-miri-for-unsafe-code.md)                | Miri for unsafe code            | Tests for `unsafe` code also run under Miri, a nightly-only tool that detects undefined behaviour     |
| [0009](docs/adr/0009-opt-in-component-trait.md)              | Opt-in component trait          | Each component type must implement `Component` by hand, and the compiler rejects every other type     |
| [0010](docs/adr/0010-hand-written-gltf-reader.md)            | Hand-written glTF reader        | The developer writes the glTF reader, and `serde_json` turns its JSON text into Rust structs          |
| [0011](docs/adr/0011-glam-for-vector-maths.md)               | glam for vector maths           | Vectors, matrices and quaternions come from glam, and the developer builds each matrix that Sarx uses |
| [0012](docs/adr/0012-bytemuck-for-gpu-bytes.md)              | bytemuck for GPU bytes          | Each type that the GPU reads derives `Pod`, and bytemuck casts it to bytes without copying it         |
| [0013](docs/adr/0013-change-detection.md)                    | Change detection                | `enki` does not track changes, and a system reports sparse changes as events                          |
| [0014](docs/adr/0014-systems-declare-their-access.md)        | Systems declare their access    | A scheduler runs systems side by side where the reads and writes that they declare do not conflict    |
| [0015](docs/adr/0015-flat-world.md)                          | A flat world                    | The land is a flat heightmap, a grid of terrain heights, as in Black & White 2                        |
| [0016](docs/adr/0016-central-ai.md)                          | Central AI                      | A planner decides for a whole group, such as a village, and each villager carries out a simple task   |
| [0017](docs/adr/0017-one-gate-for-every-change.md)           | One gate for every change       | Every change to the world arrives as a request, which one gate checks against the level's rules       |
| [0018](docs/adr/0018-one-level-format.md)                    | One level format                | The game loads generated and hand-built levels from data in one format, through the same code         |
| [0019](docs/adr/0019-stylised-art.md)                        | Stylised art                    | Sarx looks stylised, as Astro Bot does, and builds each villager from separate rigid parts            |
| [0020](docs/adr/0020-hand-poses-as-target-keyframes.md)      | Hand poses as target keyframes  | Each hand pose is a one-keyframe glTF animation, and each bone turns towards it by slerp              |
| [0021](docs/adr/0021-scale-target.md)                        | Scale target                    | Sarx holds up to 50,000 villagers and 500,000 entities on a mid-level laptop                          |
| [0022](docs/adr/0022-telekinetic-hand.md)                    | Telekinetic hand                | The hand lifts by telekinesis, hovers above every object, and pushes objects only when it strikes     |
| [0023](docs/adr/0023-rigid-creature-that-places-its-feet.md) | A creature that places its feet | The hairless creature is built from rigid parts, and code places each foot along a planned route      |
| [0024](docs/adr/0024-tiered-materials.md)                    | Tiered materials                | Players gather better materials to make better things, and each person is made of one material        |
