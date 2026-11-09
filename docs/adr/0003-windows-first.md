---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# Windows First

## Context and Problem Statement

Sarx needs a platform that it is built and tested on. It also needs a GPU that it can expect every
player to have.

wgpu describes what a GPU can do as a set of limits. One limit, for example, is the widest texture
that the GPU can create. When the renderer opens the GPU, it asks wgpu for one set of limits. wgpu
then refuses any GPU that falls short of that set.

The limits reach the window as well. The surface of the window supplies the image that each frame
is drawn into, and that image is a texture. The limit on texture size therefore caps the size of the
window too.

The choice of platform and GPU decides two things. It decides the limits that the renderer asks for.
It also decides the toolchain that a developer installs.

The two candidates are a Windows desktop and a phone.

A phone means an Android build. That build needs three things:

- an entry point that Android calls in place of `main`
- a Gradle project that packages the app
- the Android SDK and NDK on the developer's machine

A phone also means holding the renderer to the limits that wgpu guarantees on OpenGL ES 3.1. Those
limits cap a texture at 2048 pixels on each side. A window on a 1440p or 4K monitor is wider than
that, and its surface then fails to configure.

Which platform does Sarx target, and what GPU does it expect?

## Decision Drivers

- The developer's interest lies in the renderer and the engine, rather than in packaging an app for
  each platform.
- Sarx runs in full on a mid-level laptop, using the integrated GPU of that laptop.
- A port to another platform stays possible later, though none is planned now.

## Considered Options

- Windows first, with the WebGPU baseline limits
- Mobile first, with the OpenGL ES 3.1 limits

## Decision Outcome

Chosen option: "Windows first, with the WebGPU baseline limits", because it keeps the work on the
renderer and the engine, and the baseline limits fit a mid-level laptop.

Sarx is built and tested on Windows. The renderer requests `Limits::default()`, which is the
baseline of the WebGPU standard. The integrated GPU in a mid-level laptop meets that baseline.

The code that a port would need stays in place. It consists of two pieces:

- `Renderer::suspend` and `Renderer::resume`, with the code in `App::resumed` and `App::suspended`
  that calls them. On Windows, winit calls `resumed` once and never calls `suspended`. This code
  therefore runs only on a platform that can take the window away from a running app. Android does
  this when the player switches to another app.
- The display handle that `GpuContext::new` passes to the wgpu instance. wgpu uses it only when it
  runs on OpenGL. A port could need OpenGL on a device without Vulkan.

### Consequences

- Good, because a developer installs only Rust and the Visual Studio Build Tools to build Sarx.
- Good, because the renderer can create 2D textures up to 8192 pixels on each side.
- Good, because the renderer can give a shader up to 64 KiB of constant data in one uniform binding.
  The matrices of the camera are one example of such data.
- Good, because a later port starts with the suspend and resume code and the display handle already
  in place.
- Bad, because a port to a phone would request `Limits::downlevel_defaults()`. That is the set of
  limits that wgpu guarantees on OpenGL ES 3.1. The renderer would then keep its textures and its
  window within 2048 pixels on each side.
- Bad, because the suspend and resume code first runs on the platform that a port targets. Until
  then, the code stays untested.

### Confirmation

`GpuContext::new` in `crates/amaterasu/src/renderer.rs` requests `Limits::default()`.

## Pros and Cons of the Options

### Windows First, with the WebGPU Baseline Limits

- Good, because every build and every test runs on the developer's own machine.
- Good, because the limits suit the GPUs that Sarx expects.
- Bad, because a later port has to fit the renderer within the limits of the new platform.

### Mobile First, with the OpenGL ES 3.1 Limits

- Good, because one set of limits fits nearly every native platform that wgpu supports, phones
  included.
- Bad, because each Android build needs the SDK, the NDK and Gradle. Each test run also needs an
  emulator or a phone.
- Bad, because the limits cap textures and the window at 2048 pixels on each side. A 1440p or 4K
  monitor is wider than that.
- Bad, because supporting Android takes time away from the renderer and the engine.
