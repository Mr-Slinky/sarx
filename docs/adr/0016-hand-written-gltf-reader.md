---
status: accepted
date: 2026-11-09
decision-makers: Kheagen Haskins
---

# Hand-Written glTF Reader

## Context and Problem Statement

The god's hand reaches Sarx as `assets/hand-model.glb`, a binary glTF file that Blender exports. A
binary glTF file has three parts:

1. A 12-byte header, which identifies the file as glTF and gives its length.
2. A JSON text, which describes the contents of the file.
3. A block of bytes, which stores the numbers themselves.

The JSON text never contains a vertex. Instead, it gives directions into the block of bytes. One
entry, for example, states that the vertex positions are 13,192 groups of three floats, which start
at a given byte offset.

Sarx needs four things from the file. They are the vertices of the mesh, the bone that each vertex
follows, the skeleton, and the poses that ADR 0014 stores as one-keyframe animations.

How does Sarx read a glTF file?

## Decision Drivers

- Under the second goal in the README, the developer builds layers by hand to understand the
  graphics pipeline.
- Reading the block of bytes follows the same rules as a vertex buffer in wgpu. Both state a
  byte offset, a stride and a number type for each attribute.
- Sarx reads only the files that its own Blender export writes. The reader therefore has to handle
  the features of that export alone.
- Parsing JSON text teaches nothing about graphics.

## Considered Options

- The `gltf` crate
- A hand-written reader, with `serde_json` for the JSON text
- A hand-written reader, with a hand-written JSON parser

## Decision Outcome

Chosen option: "A hand-written reader, with `serde_json` for the JSON text", because the developer
then writes every part that concerns graphics, and a library handles the part that does not.

The developer writes the code that splits the file into its three parts. The developer also writes
the code that follows each direction in the JSON text into the block of bytes. `serde_json` turns
the JSON text into Rust structs, which the developer declares with only the fields that Sarx reads.
`serde` supplies the derive macro for those structs.

The reader handles the features that the Blender export uses. Each other feature of glTF must wait
until a file that Sarx loads needs it.

The reader returns each animation in the form that glTF stores it. An animation is a list of
channels. Each channel moves one node, which is a bone or a separate part of a model. A channel
changes the position, the rotation or the scale of its node. It stores the time of each keyframe,
along with the value at that time. It also states how the value moves from one keyframe to the next.

The reader returns every channel in the file, and the code that uses an animation picks the channels
that it needs. The pose code for the hand takes a one-keyframe animation, keeps its rotation
channels, and builds the target pose that ADR 0014 sets out. Villagers, built from rigid parts under
ADR 0012, move whole parts through time. Their code must play every channel over the length of a
clip, positions and scales included.

Each new kind of animation then adds code on top of the reader. The form of what the reader returns
stays the same.

The reader lives in a crate of its own, `hermes`. `zurvan` must not depend on `hermes`, since the
reader is graphics code.

### Consequences

- Good, because the developer learns how a mesh is laid out in bytes. A vertex buffer layout in
  wgpu uses the same terms.
- Good, because the developer learns how the skeleton, the inverse bind matrices and the poses are
  stored. Skinning the hand needs that knowledge.
- Good, because the structs declare only the fields that Sarx reads, which keeps them short.
- Good, because the villagers can animate position and scale through time without a change to what
  the reader returns. Their code reads the same channels that the hand's pose code reads.
- Bad, because the reader rejects a file that uses a feature beyond the Blender export, such as
  sparse accessors or an external buffer.
- Bad, because Sarx takes on two crates, `serde` and `serde_json`.

### Confirmation

Nothing in the code embodies this decision yet. Once the hand loads, a reader confirms it with three
checks:

- The workspace depends on `serde` and `serde_json`, and does not depend on `gltf`.
- The code that reads the block of bytes is Sarx's own.
- The reader returns each animation as its channels. The pose code for the hand, rather than the
  reader, picks out the rotations.

## Pros and Cons of the Options

### The `gltf` Crate

The `gltf` crate reads every part of a glTF file. It offers calls such as `read_positions()`,
which return the vertex data as Rust values.

- Good, because it reads any valid glTF file, including features beyond the Blender export.
- Good, because it would save the developer the work of reading the block of bytes.
- Bad, because it hides how the vertex data is laid out in bytes. That layout is the part of glTF
  that teaches the most about the GPU.

### A Hand-Written Reader, with `serde_json` for the JSON Text

- Good, because the developer writes every part of the reader that concerns graphics.
- Good, because `serde_json` is the standard JSON library for Rust.
- Bad, because the reader handles the Blender export alone.

### A Hand-Written Reader, with a Hand-Written JSON Parser

- Good, because Sarx would take on no library for glTF.
- Bad, because a JSON parser runs to a few hundred lines that concern text parsing alone. The
  developer does not want to build it.
