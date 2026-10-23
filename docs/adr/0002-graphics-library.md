---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# wgpu as the Graphics Library

## Context and Problem Statement

Sarx draws its world on the GPU. Windows offers two modern graphics APIs, DirectX 12 and Vulkan, and
Sarx is built and tested on Windows, as ADR 0003 records. Sarx's renderer talks to the GPU through one
of these APIs, or through a library that works above them.

The developer builds the renderer by hand to learn the graphics pipeline. That pipeline consists of
the GPU setup, the shaders, the buffers, the render pipelines and the draw calls.

Which API, or which library, does the renderer use to reach the GPU?

## Decision Drivers

- The pipeline, shaders, buffers and draw calls are the parts that Sarx sets out to teach, meaning
  Sarx's own code has to write them.
- Sarx's code reaches its first triangle, and every later feature, with setup code that teaches the
  pipeline rather than memory management.
- A mistake in a call produces a message that explains it.
- The chosen option has to accept the window handles that winit provides, as ADR 0001 records.
- The renderer leaves a later port open, as ADR 0003 sets out.

## Considered Options

- wgpu
- DirectX 12, called directly
- Vulkan, called directly
- OpenGL

## Decision Outcome

Chosen option: "wgpu", because it leaves the whole graphics pipeline to Sarx's own code, checks every
call with a readable message, and teaches the concepts that DirectX 12 and Vulkan share.

### Consequences

- Good, because wgpu follows the WebGPU standard, meaning its concepts carry over to the modern
  native APIs that it runs on.
- Good, because wgpu checks every call against its rules and reports a mistake with a readable
  message, where a native API would crash or draw garbage.
- Good, because wgpu accepts any window that provides raw window handles, leaving `sarx-render`
  independent of winit.
- Good, because a port runs the same renderer on the new platform's API.
- Bad, because wgpu changes its API in most major releases. Each upgrade therefore follows the
  changelog, and a developer checks the real definitions in the crate source for the version that
  `Cargo.lock` records.
- Bad, because shaders are written in WGSL, WebGPU's shading language, which has fewer tutorials than
  GLSL or HLSL.

### Confirmation

`cargo tree -i wgpu` lists `sarx-render` as the Sarx crate that depends on wgpu.

## Pros and Cons of the Options

### wgpu

wgpu is a Rust implementation of the WebGPU API. It runs on DirectX 12, Vulkan, Metal and OpenGL,
choosing the one that each platform supports.

- Good, because Sarx's code still creates every pipeline, shader, buffer and draw call.
- Good, because the Rust graphics community uses it widely, and its repository contains a set of
  runnable examples.
- Good, because it validates every call and reports errors in plain messages.
- Neutral, because it hides the memory management and synchronisation that DirectX 12 and Vulkan
  expose.
- Bad, because its API changes in most major releases.

### DirectX 12, Called Directly

Sarx would call DirectX 12 through Microsoft's `windows` crate.

- Good, because it exposes everything the GPU does on Windows, including memory management and
  synchronisation.
- Good, because Microsoft documents it in full, with samples for every feature.
- Bad, because it requires a large amount of setup code before the first triangle, most of it about
  memory and synchronisation rather than the pipeline.
- Bad, because a mistake often crashes the program or draws garbage, with no error message unless
  the debug layer is enabled.
- Bad, because it runs on Windows and Xbox alone, meaning a port would need a second renderer.

### Vulkan, Called Directly

Sarx would call Vulkan through a thin Rust binding, such as `ash`.

- Good, because it exposes everything the GPU does, including memory management and synchronisation.
- Good, because it runs on Windows, Linux and Android.
- Bad, because Vulkan requires a large amount of setup code before the first triangle, most of it
  about memory and synchronisation rather than the pipeline.
- Bad, because a mistake often crashes the program or draws garbage, with no error message unless
  the validation layers are installed.

### OpenGL

- Good, because decades of tutorials cover it.
- Bad, because its design comes from older GPUs, keeping global state that the modern APIs replace.
- Bad, because the concepts that it teaches differ from the ones that DirectX 12, Vulkan and Metal
  share.

## More Information

Sarx adopts wgpu 30.0.1, the latest stable release on the date of the original decision. The wgpu
repository's `examples` folder and its `CHANGELOG.md` are the references for its API between
versions.
