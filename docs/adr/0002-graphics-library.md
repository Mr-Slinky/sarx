---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# wgpu as the Graphics Library

## Context and Problem Statement

Sarx draws its world on the GPU. To do that, the renderer sends its commands to the GPU through a
graphics API.

Sarx is built and tested on Windows, as ADR 0003 records. Windows offers two modern graphics APIs,
DirectX 12 and Vulkan. It also offers OpenGL, which is older. The renderer can call one of these APIs
itself. It can also use a library that calls them on its behalf.

The developer builds the renderer by hand to learn the graphics pipeline. That pipeline consists of
the GPU setup, the shaders, the buffers, the render pipelines and the draw calls.

Which API, or which library, does the renderer use to reach the GPU?

## Decision Drivers

- The pipeline, shaders, buffers and draw calls are the parts that Sarx sets out to teach. Sarx
  therefore writes them in its own code.
- The setup code that the developer writes teaches the pipeline rather than memory management. This
  applies to the first triangle and to every later feature.
- A mistake in a call produces a message that explains it.
- The chosen option accepts the window handles that winit provides, as ADR 0001 records.
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

- Good, because wgpu follows the WebGPU standard. The concepts of that standard carry over to the
  modern native APIs that wgpu runs on.
- Good, because wgpu checks every call against its rules. It reports each mistake with a readable
  message. A native API reports a mistake only once its debug or validation layers are switched on.
  Otherwise, the program often crashes or draws garbage.
- Good, because wgpu accepts any window that provides raw window handles. As a result, `amaterasu`
  stays independent of winit.
- Good, because a port runs the same renderer on the API of the new platform. The renderer then works
  within the limits that ADR 0003 sets out for that platform.
- Bad, because wgpu changes its API in most major releases. A developer follows the changelog for
  each upgrade. They also check the real definitions in the crate source, for the version that
  `Cargo.lock` records.
- Bad, because shaders are written in WGSL, the shading language of WebGPU. WGSL has fewer tutorials
  than the two older shading languages, GLSL for OpenGL and Vulkan, and HLSL for DirectX.

### Confirmation

`cargo tree -i wgpu --depth 1` lists `amaterasu` as the only Sarx crate that depends on wgpu
directly.

## Pros and Cons of the Options

### wgpu

wgpu is a Rust implementation of the WebGPU API. It runs on DirectX 12, Vulkan, Metal and OpenGL. On
each platform, it chooses an API that the platform supports.

- Good, because Sarx still creates every pipeline, shader, buffer and draw call.
- Good, because the Rust graphics community appears to use it widely.
- Good, because its repository contains a set of runnable examples.
- Good, because it validates every call and reports errors in plain messages.
- Neutral, because it hides memory management and synchronisation. DirectX 12 and Vulkan expose
  both.
- Bad, because its API changes in most major releases.

### DirectX 12, Called Directly

Sarx would call DirectX 12 through the `windows` crate from Microsoft.

- Good, because it exposes everything that the GPU does on Windows. That includes memory management
  and synchronisation.
- Good, because Microsoft documents it in full, with samples for every feature.
- Bad, because it needs a large amount of setup code before the first triangle. Most of that code
  manages memory and synchronisation rather than the pipeline.
- Bad, because a mistake often crashes the program or draws garbage. An error message appears only
  once the debug layer is enabled.
- Bad, because it runs on Windows and Xbox alone. A port would therefore need a second renderer.

### Vulkan, Called Directly

Sarx would call Vulkan through a thin Rust binding, such as `ash`.

- Good, because it exposes everything that the GPU does. That includes memory management and
  synchronisation.
- Good, because it runs on Windows, Linux and Android.
- Bad, because it needs a large amount of setup code before the first triangle. Most of that code
  manages memory and synchronisation rather than the pipeline.
- Bad, because a mistake often crashes the program or draws garbage. An error message appears only
  once the validation layers are installed.

### OpenGL

Sarx would call OpenGL through a Rust binding, such as `glow`.

- Good, because decades of tutorials cover it.
- Bad, because its design dates from older GPUs. Every call reads or changes one global state.
  DirectX 12, Vulkan and Metal store the same settings in objects instead. The program creates those
  objects and passes them around.
- Bad, because the concepts that it teaches differ from the ones that DirectX 12, Vulkan and Metal
  share.

## More Information

Sarx adopted wgpu 30.0.1 on 18 October 2026, when the developer first chose wgpu. It was the latest
stable release at that time. When the API changes between versions, a developer checks the
`examples` folder and `CHANGELOG.md` in the wgpu repository.
