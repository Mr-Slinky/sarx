---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# Windows First

## Context and Problem Statement

Sarx needs a platform that it is built and tested on, and a GPU that it can expect every player to
have. Those two choices set the limits that the renderer asks the GPU for, and the toolchain that a
developer installs.

Running on a phone means an Android build: an entry point that Android calls in place of `main`, a
Gradle project that packages the app, and the Android SDK and NDK on the developer's machine. It also
means holding the renderer to the limits that wgpu guarantees on OpenGL ES 3.1, which cap a texture
at 2048 pixels on each side. A window wider than that, on a 1440p or 4K monitor, then fails to
configure its surface.

Which platform does Sarx target, and what GPU does it expect?

## Decision Drivers

- The developer's interest lies in the renderer and the engine, rather than in packaging an app for
  each platform.
- Sarx runs in full on a mid-level laptop, using its integrated GPU.
- A port to another platform stays possible later, without being planned now.

## Considered Options

- Windows first, with the WebGPU baseline limits
- Mobile first, with the OpenGL ES 3.1 limits

## Decision Outcome

Chosen option: "Windows first, with the WebGPU baseline limits", because it keeps the work on the
renderer and the engine, and the baseline limits fit a mid-level laptop.

Sarx is built and tested on Windows. The renderer requests `Limits::default()`, the WebGPU standard's
baseline, which the integrated GPU in a mid-level laptop meets.

The code that a port would need stays in place:

- `Renderer::suspend` and `Renderer::resume`, with the `App::resumed` and `App::suspended` code that
  calls them. winit calls `resumed` once on Windows and never calls `suspended`, meaning this code
  runs only on a platform with an app lifecycle, such as Android.
- The display handle that `GpuContext::new` passes to the wgpu instance. Only the OpenGL backend uses
  it, and DirectX 12 and Vulkan ignore it.

### Consequences

- Good, because a developer installs only Rust and the Visual Studio Build Tools to build Sarx.
- Good, because the renderer can create textures up to 8192 pixels on each side, and uniform buffers
  up to 64 KiB.
- Good, because the suspend and resume code and the display handle keep a later port to a smaller
  step.
- Bad, because a port to a phone would request `Limits::downlevel_defaults()` on that platform, and
  would have to fit within them.
- Bad, because nothing tests the suspend and resume code on Windows.

### Confirmation

`GpuContext::new` in `crates/sarx-render/src/renderer.rs` requests `Limits::default()`.

## Pros and Cons of the Options

### Windows First, with the WebGPU Baseline Limits

- Good, because every build and every test runs on the developer's own machine.
- Good, because the limits suit the GPUs that Sarx expects.
- Bad, because a port later on has to fit the platform's own limits.

### Mobile First, with the OpenGL ES 3.1 Limits

- Good, because one set of limits fits every platform that wgpu supports.
- Bad, because each Android build needs the SDK, the NDK, a packaging tool and an emulator or a phone.
- Bad, because the limits cap textures and the window at 2048 pixels on each side, below many desktop
  monitors.
- Bad, because the work of supporting Android draws time from the renderer and the engine.
