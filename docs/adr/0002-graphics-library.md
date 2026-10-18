---
status: accepted
date: 2026-10-18
decision-makers: Kheagen Haskins
---

# wgpu as the Graphics Library

## Context and Problem Statement

Sarx draws its world on the GPU. Each platform offers its own graphics API: DirectX 12 on Windows,
Vulkan and OpenGL ES on Android, and Metal on macOS and iOS. Sarx's renderer talks to the GPU through
one of these APIs, or through a library that sits above several of them.

The developer builds the renderer by hand to learn the graphics pipeline. That pipeline consists of
the GPU setup, the shaders, the buffers, the render pipelines and the draw calls.

Which API, or which library, does the renderer use to reach the GPU?

## Decision Drivers

- The mobile-first design decision requires the renderer to run on Android phones as well as on the
  desktop.
- The pipeline, shaders, buffers and draw calls are the parts that Sarx sets out to teach, meaning
  Sarx's own code has to write them.
- One renderer has to run on every platform that Sarx plans to ship.
- The chosen option has to accept the window handles that winit provides, as ADR 0001 records.

## Considered Options

- wgpu
- Vulkan, called directly
- OpenGL and OpenGL ES
- A separate native API for each platform

## Decision Outcome

Chosen option: "wgpu", because it leaves the whole graphics pipeline to Sarx's own code while
handling the differences between Vulkan, Metal and DirectX 12 itself.

### Consequences

- Good, because one renderer runs on Windows, Android and Apple platforms.
- Good, because wgpu follows the WebGPU standard, meaning its concepts carry over to the modern
  native APIs that it runs on.
- Good, because wgpu checks every call against its rules and reports a mistake with a readable
  message, where a native API would crash or draw garbage.
- Good, because wgpu accepts any window that provides raw window handles, leaving `sarx-render`
  independent of winit.
- Bad, because wgpu changes its API in most major releases. Each upgrade therefore follows the
  changelog, and a developer checks the real definitions in the crate source for the version that
  `Cargo.lock` records.
- Bad, because shaders are written in WGSL, WebGPU's shading language, which has fewer tutorials than
  GLSL or HLSL.

### Confirmation

`cargo tree -i wgpu` lists `sarx-render` as the Sarx crate that depends on wgpu.

## Pros and Cons of the Options

### wgpu

wgpu is a Rust implementation of the WebGPU API. It runs on Vulkan, Metal, DirectX 12 and OpenGL ES,
choosing the one that each platform supports.

- Good, because Sarx's code still creates every pipeline, shader, buffer and draw call.
- Good, because it covers every target that Sarx plans to ship with one API.
- Good, because the Rust graphics community uses it widely, and its repository contains a set of
  runnable examples.
- Good, because it validates every call and reports errors in plain messages.
- Neutral, because it hides the memory management and synchronisation that Vulkan exposes.
- Bad, because its API changes in most major releases.

### Vulkan, Called Directly

Sarx would call Vulkan through a thin Rust binding, such as `ash`.

- Good, because it exposes everything the GPU does, including memory management and synchronisation.
- Good, because Android supports it on most current phones.
- Bad, because Apple platforms support Vulkan only through a translation layer, MoltenVK.
- Bad, because Vulkan requires a large amount of setup code before the first triangle, most of it
  about memory and synchronisation rather than the pipeline.
- Bad, because a mistake often crashes the program or draws garbage, with no error message unless
  the validation layers are installed.

### OpenGL and OpenGL ES

- Good, because every Android phone supports OpenGL ES, and decades of tutorials cover OpenGL.
- Bad, because its design comes from older GPUs, keeping global state that the modern APIs replace.
- Bad, because Apple has deprecated OpenGL on its platforms.
- Bad, because the concepts that it teaches differ from the ones that Vulkan, Metal and DirectX 12
  share.

### A Separate Native API for Each Platform

Sarx would write one renderer for DirectX 12, another for Vulkan, and another for Metal.

- Good, because each renderer uses its platform's API in full.
- Bad, because every feature has to be written once for each platform.
- Bad, because the three APIs differ in detail, meaning most of the extra work is translation rather
  than graphics.

## More Information

Sarx adopts wgpu 30.0.1, the latest stable release on the date of this decision. The wgpu
repository's `examples` folder and its `CHANGELOG.md` are the references for its API between
versions.
