---
status: accepted
date: 2026-11-10
decision-makers: Kheagen Haskins
---

# bytemuck for GPU Bytes

## Context and Problem Statement

wgpu copies data to the GPU as a slice of bytes, a `&[u8]`. The GPU then reads those bytes by the
layout that the pipeline states, such as a byte offset for each vertex attribute.

Today, `Mesh` stores each vertex as `[f32; 3]`. Its `to_bytes` method builds a new `Vec<u8>`, and
copies each coordinate into it with `to_ne_bytes`.

The hand step brings two more kinds of data. A vertex becomes a `Vertex` struct, with a position and
a normal. The camera also sends its matrices to the shader in a uniform buffer, which is a small GPU
buffer that every vertex can read.

Copying field by field needs more code with every new field. A direct cast avoids that code, by
treating the memory of a struct as bytes without copying it.

A direct cast is safe only for some types. Where a struct has padding, the compiler leaves gaps
between its fields, and those gaps hold uninitialised memory. Reading uninitialised memory as `u8`
is undefined behaviour.

How does Sarx turn vertices and uniforms into bytes?

## Decision Drivers

- The README asks of every library whether it brings its own architecture, what it hides, and how
  much work it does.
- A cast of a type with padding is undefined behaviour, and undefined behaviour can pass every test.
  The check for padding must therefore happen at compile time.
- Under ADR 0013, the tests for `unsafe` code also run under Miri. Each `unsafe` cast that Sarx
  writes itself adds to that work.
- `Vertex`, the uniforms and later instance data all need the same conversion.

## Considered Options

- bytemuck
- A `to_bytes` method on each type
- An `unsafe` cast written by hand

## Decision Outcome

Chosen option: "bytemuck", because its derive macro rejects padding at compile time, and its casts
copy nothing.

A type that the GPU reads takes `#[repr(C)]`, along with two derived traits from bytemuck:

- `Zeroable` states that a value with every byte set to zero is valid.
- `Pod`, short for plain old data, states that every pattern of bytes is a valid value of the type.
  The derive macro rejects a type that has padding, or a field that is not `Pod` itself.

Two functions then return the bytes. `bytemuck::cast_slice` returns a `&[u8]` over the memory of a
slice, such as every vertex of a mesh. `bytemuck::bytes_of` returns a `&[u8]` over one value, such as
the uniforms of the camera.

Sarx adds bytemuck with its `derive` feature. Sarx also turns on the `bytemuck` feature of glam,
which ADR 0017 sets out. A `Mat4` field then counts as `Pod` inside a uniform struct.

### Consequences

- Good, because a type with padding fails to compile, before it can reach the GPU.
- Good, because a cast returns the bytes in place. A mesh of 13,192 vertices is not copied a second
  time before wgpu copies it to the GPU.
- Good, because every type that the GPU reads uses the same two lines.
- Bad, because bytemuck hides the `unsafe` cast. The developer does not write the code that turns a
  struct into bytes.
- Bad, because the developer must lay out each struct with padding in mind. A uniform struct, for
  example, must add a padding field of its own where WGSL aligns a `vec3` to 16 bytes.

### Confirmation

Nothing in the code embodies this decision yet. Once `Vertex` exists, a reader confirms it with two
checks:

- `Vertex` and each uniform struct take `#[repr(C)]`, and derive `Pod` and `Zeroable`.
- No type that the GPU reads has a `to_bytes` method of its own.

## Pros and Cons of the Options

### bytemuck

bytemuck is a small library of safe casts between types and bytes.

- Good, because its derive macro checks each type at compile time.
- Good, because wgpu examples and most Rust graphics code use it.
- Bad, because it adds a derive macro to the build.

### A `to_bytes` Method on Each Type

Each type would copy its fields into a `Vec<u8>`, as `Mesh::to_bytes` does today.

- Good, because the code contains no `unsafe` block.
- Bad, because every new field needs a new line in `to_bytes`. A field left out shifts every offset
  after it, and nothing reports the mistake until the picture comes out wrong.
- Bad, because every call copies the data once before wgpu copies it again.

### An `unsafe` Cast Written by Hand

Sarx would call `std::slice::from_raw_parts` over the memory of each slice.

- Good, because the developer would write the cast, and see how a struct lies in memory.
- Bad, because nothing checks for padding. A struct with a gap compiles, and the cast is then
  undefined behaviour.
- Bad, because the tests for every such cast must also run under Miri, as ADR 0013 sets out.
