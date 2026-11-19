---
status: proposed
date: 2026-11-19
decision-makers: Kheagen Haskins
---

# The gltf Crate for Reading glTF

## Context and Problem Statement

The god's hand reaches Sarx as `assets/hand-model.glb`, a binary glTF file that Blender exports. A
binary glTF file has three parts:

1. A 12-byte header, which identifies the file as glTF and gives its length.
2. A JSON text, which describes the contents of the file.
3. A block of bytes, which stores the numbers themselves.

The JSON text never contains a vertex. Instead, it gives directions into the block of bytes. Each
direction is an accessor, which states where a list of values starts in the block, how far apart
the values lie, and what type of number each one is.

Sarx needs four things from the file. They are the vertices of the mesh, the bone that each vertex
follows, the skeleton, and the poses that ADR 0020 stores as one-keyframe animations.

Until this decision, `hermes` read the file by hand, with `serde_json` for the JSON text. The
developer wrote the code that splits the file into its three parts, and the code that follows each
accessor into the block of bytes. That code reads the vertices of the mesh, and runs to about 3,400
lines with its tests.

The other three things remain. The hand's file contains a skeleton of 22 bones, a skin that ties
each vertex to those bones, and 9 animations of 66 channels each. Reading them by hand means more
accessor types, more of the JSON text, and more checks on each. The work that remains repeats the
lesson that the mesh already taught.

How does Sarx read a glTF file?

## Decision Drivers

- The developer builds layers by hand to understand the graphics pipeline, as the
  [Built by Hand](../../README.md#built-by-hand) section of the README sets out. That section also
  expects a library to replace a hand-built layer once the layer has taught what it was built to
  teach.
- The hand-written reader has taught how a mesh is laid out in bytes. A vertex buffer in wgpu uses
  the same terms: a byte offset, a stride and a number type for each attribute.
- The skin and the poses were the next work on the hand, and the hand-written reader did not read
  them.
- The renderer and the program that runs the game should depend on Sarx's own types for a mesh, a
  skeleton and a pose. A change of glTF library then stays inside one crate.

## Considered Options

- The `gltf` crate, behind `hermes`
- The `gltf` crate, called directly by each crate that reads a model
- The hand-written reader, extended to read the skin and the animations

## Decision Outcome

Chosen option: "The `gltf` crate, behind `hermes`", because the hand-written reader has already
taught the byte layout, and the crate reads the skin and the animations that the hand needs next.

The `gltf` crate reads every part of a glTF file. `hermes` passes the file to the crate, and turns
what the crate returns into Sarx's own types: `MeshData` for the shape, `SkinData` and `BoneData`
for the skeleton, and `PoseData` for each pose. `amaterasu` and `atum` call `hermes`, and do not
depend on `gltf`.

The function below shows how `hermes` could read the vertex positions of a mesh through the crate.
A reader that the crate builds follows each accessor into the block of bytes. The function returns
`None` wherever the file lacks a part. The real code, `read_mesh` in `crates/hermes/src/model.rs`,
returns a `GltfError` that names the part instead.

```rust
fn read_positions(file: &[u8]) -> Option<Vec<[f32; 3]>> {
    // Splits the file into its JSON text and its block of bytes
    let glb = gltf::Gltf::from_slice(file).ok()?;
    let binary = glb.blob.as_deref()?;

    // Builds a reader for the first primitive, which finds every buffer in the block of bytes
    let mesh = glb.meshes().next()?;
    let primitive = mesh.primitives().next()?;
    let reader = primitive.reader(|_buffer| Some(binary));

    // Follows the accessor for the positions, and collects one position for each vertex
    let positions = reader.read_positions()?;
    Some(positions.collect())
}
```

`hermes` turns off the crate's default features. Those features load a `.gltf` file whose
buffers and textures lie in separate files, and they bring in a library for decoding images. Sarx
loads `.glb` files alone, and the reader above works without those features.

`hermes` will return each animation in the form that glTF stores it. An animation is a list of
channels. Each channel moves one node, which is a bone or a separate part of a model. A channel
changes the position, the rotation or the scale of its node. It stores the time of each keyframe,
along with the value at that time. It also states how the value moves from one keyframe to the next.

`hermes` will return every channel in the file, and the code that uses an animation picks the
channels that it needs. The pose code for the hand takes a one-keyframe animation, keeps its
rotation channels, and builds the target pose that ADR 0020 sets out. Villagers, built from rigid
parts under ADR 0019, move whole parts through time. Their code must play every channel over the
length of a clip, positions and scales included.

Each new kind of animation then adds code on top of `hermes`. The form of what `hermes` returns
stays the same.

`zurvan` must not depend on `hermes`, since reading a model is graphics code.

### Consequences

- Good, because the hand's skin and poses come from a reader that already handles them. A trial read
  of `assets/hand-model.glb` with the crate returned 3,558 vertices, 22 bones with their inverse bind
  matrices, and 9 animations.
- Good, because the crate reads any valid glTF file. A model from another tool, or one that uses
  sparse accessors, loads without new code in `hermes`.
- Good, because `amaterasu` and `atum` depend on Sarx's own types alone. Replacing the crate changes
  `hermes` and nothing else.
- Good, because the villagers can animate position and scale through time without a change to what
  `hermes` returns. Their code reads the same channels that the hand's pose code reads.
- Bad, because the crate hides how the skin and the animations are laid out in bytes. The developer
  learns their structure from the crate's types instead, which name the same parts that the glTF
  specification names.
- Bad, because Sarx takes on the `gltf` crate, and through it `gltf-json`, `gltf-derive`,
  `byteorder`, `lazy_static`, `serde` and `serde_json`.
- Bad, because the hand-written reader goes. Its code stays in the git history.

### Confirmation

`hermes` reads the file through the crate, and the hand-written reader has gone. A reader confirms
the decision with three checks:

- `hermes` depends on `gltf` with its default features turned off, as `crates/hermes/Cargo.toml`
  shows. No other crate in the workspace depends on `gltf`.
- The public types of `hermes` are Sarx's own, and none of them comes from `gltf`. One variant
  differs: `GltfError::Gltf` in `crates/hermes/src/error.rs` wraps a `gltf::Error`.
- `hermes` returns each animation as its channels. The pose code for the hand, rather than `hermes`,
  picks out the rotations. The code differs here: `read_pose` in `crates/hermes/src/pose.rs` keeps
  the rotation channels alone, and `hermes` returns each pose as a `PoseData` of rotations.

## Pros and Cons of the Options

### The `gltf` Crate, behind `hermes`

- Good, because the crate reads the skin and the animations, which the hand needs next.
- Good, because the rest of Sarx depends on `hermes` alone, and never on the crate.
- Bad, because `hermes` has to turn each type that the crate returns into one of Sarx's own.

### The `gltf` Crate, Called Directly by Each Crate That Reads a Model

- Good, because Sarx would not need the code that turns the crate's types into its own.
- Bad, because the crate's types would spread through the renderer, and replacing the crate would
  change every crate that reads a model.

### The Hand-Written Reader, Extended to Read the Skin and the Animations

- Good, because the developer would write every part of the reader.
- Good, because Sarx would depend on `serde` and `serde_json` alone.
- Bad, because the skin and the animations repeat the lesson that the mesh already taught, through
  more accessors and more of the JSON text.
- Bad, because the reader would handle only the features of the Blender export, and would reject a
  file that uses sparse accessors or an external buffer.
