---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# winit as the Window Library

## Context and Problem Statement

Sarx needs a window to draw into, and an event loop that delivers input and other events from the
operating system to the game. The event loop is the first piece of structure in `sarx-app`, and the
GPU setup and the fixed-step simulation build on it in later steps.

Sarx is built and tested on Windows, as ADR 0003 records. A port to another platform stays possible,
without being planned.

Which library, if any, opens the window and runs the event loop?

## Decision Drivers

- wgpu creates its surface from the raw window handles that each platform defines, meaning the window
  layer has to provide them.
- The window layer belongs to the operating system rather than to the graphics pipeline or the engine,
  which are the parts that Sarx sets out to teach.
- Sarx learns from the wgpu documentation and examples, and matching their window layer keeps those
  examples close to Sarx's own code.
- The window layer leaves a later port open, as ADR 0003 sets out.

## Considered Options

- winit
- A hand-written Win32 layer
- SDL3
- GLFW

## Decision Outcome

Chosen option: "winit", because it provides the window and the event loop in pure Rust, the wgpu
documentation uses it, and the same code runs on the other platforms that a port could target.

### Consequences

- Good, because the examples in the wgpu repository open their windows with the same library.
- Good, because a port to another platform keeps the same window layer.
- Good, because only `sarx-app` depends on winit, keeping the simulation and the renderer independent
  of the window layer.
- Bad, because Sarx follows winit's design: it implements winit's application trait, and winit drives
  the program by calling it.
- Bad, because winit changes its API between major releases. Each upgrade therefore follows the
  changelog, and a developer checks the real definitions in the crate source for the version that
  `Cargo.lock` records.

### Confirmation

`cargo tree -i winit` lists `sarx-app` as the only Sarx crate that depends on winit.

## Pros and Cons of the Options

### winit

winit is the standard window library for Rust. It opens windows and delivers events on Windows,
macOS, Linux, Android, iOS and the web through one API.

- Good, because the wgpu documentation and every example in the wgpu repository use it.
- Good, because it hides only operating-system plumbing.
- Good, because it reports the lifecycle events that a phone or the web sends, should Sarx be ported.
- Neutral, because its architecture follows from the platforms that it supports. Android and the web
  call into an app when an event arrives, meaning a library for them cannot let the app own the loop.
- Bad, because Sarx implements a trait, and winit calls it for each event.
- Bad, because its API changes between major releases.

### A Hand-Written Win32 Layer

Sarx would call Win32 directly to open the window and read its messages, then pass the raw window
handles to wgpu itself.

- Good, because it teaches how Windows delivers input and runs an app.
- Good, because Sarx owns every line, with no outside design to follow.
- Bad, because that knowledge belongs to the operating system rather than to the graphics pipeline or
  the engine.
- Bad, because a port would need a second layer, written from scratch for the new platform.

### SDL3

SDL3 is a C library with Rust bindings. The app runs its own loop and polls SDL for events, which
matches the classic shape of a game loop.

- Good, because the app owns the loop.
- Good, because it supports the same platforms as winit.
- Bad, because each build compiles and links a C library.
- Bad, because it overlaps with wgpu, since SDL also offers its own GPU API.
- Bad, because the wgpu documentation and examples use winit instead.

### GLFW

GLFW is a C library with Rust bindings, built for desktop graphics programs.

- Good, because its API is small and well known from C and C++ graphics tutorials.
- Bad, because each build compiles and links a C library.
- Bad, because it supports the desktop alone, which closes off a port to a phone.
- Bad, because the wgpu documentation and examples use winit instead.

## More Information

Sarx adopts winit 0.30.13, the latest stable release on the date of the original decision.
