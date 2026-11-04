---
status: accepted
date: 2026-10-23
decision-makers: Kheagen Haskins
---

# winit as the Window Library

## Context and Problem Statement

Sarx needs a window to draw into. It also needs an event loop. An event loop is the code that
receives input and other events from the operating system and passes them on to the game.

Structure is code that other code builds on. The event loop is the first piece of structure in
`sarx-app`. The GPU setup already builds on it. The fixed-step simulation will build on it in a later step.

Sarx is built and tested on Windows, as ADR 0003 records. A port to another platform stays possible,
though none is planned.

Which library, if any, opens the window and runs the event loop?

## Decision Drivers

- wgpu creates its surface from raw window handles. Each platform defines handles of its own. The
  window layer therefore has to provide them.
- Opening a window and reading its events is work for the operating system. Sarx sets out to teach
  the graphics pipeline and the engine instead.
- The developer learns from the wgpu documentation and examples. Those examples use one window
  library. Where Sarx uses the same library, its code looks like the code that the developer is
  reading.
- ADR 0003 keeps a later port possible. The window layer therefore has to leave one open.

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
- Good, because only `sarx-app` depends on winit. The simulation and the renderer therefore stay
  independent of the window layer.
- Bad, because Sarx follows the design of winit. `App` implements the `ApplicationHandler` trait from
  winit, and winit calls the methods of that trait for each event.
- Bad, because winit changes its API between releases, such as from 0.29 to 0.30. A developer
  follows the changelog for each upgrade. They also check the real definitions in the crate source,
  for the version that `Cargo.lock` records.

### Confirmation

`cargo tree -i winit` lists `sarx-app` as the only Sarx crate that depends on winit.

## Pros and Cons of the Options

### winit

winit is likely the most widely used window library for Rust. It opens windows and delivers events
through one API. That API works on Windows, macOS, Linux, Android, iOS and the web.

- Good, because the wgpu documentation and every example in the wgpu repository use it.
- Good, because it handles only windows and events. The graphics pipeline and the engine stay with
  Sarx.
- Good, because it reports the events that a phone or the web sends when an app is paused or
  resumed. A port would need those events.
- Neutral, because its architecture follows from the platforms that it supports. On Android and the
  web, the platform calls into the app when an event arrives. A library for those platforms therefore
  cannot let the app own the loop.
- Bad, because Sarx has to fit the design of winit. Sarx implements `ApplicationHandler`, and winit
  calls its methods for each event.
- Bad, because its API changes between releases, such as from 0.29 to 0.30.

### A Hand-Written Win32 Layer

Sarx would call Win32 directly to open the window and read its messages. Sarx itself would then pass
the raw window handles to wgpu.

- Good, because it teaches how Windows delivers input and runs an app.
- Good, because Sarx owns every line and follows its own design.
- Bad, because that knowledge concerns the operating system. Sarx sets out to teach the graphics
  pipeline and the engine.
- Bad, because a port would need a second layer, written from scratch for the new platform.

### SDL3

SDL3 is a C library with Rust bindings. The app runs its own loop, and asks SDL for new events on
each pass. This matches the classic shape of a game loop.

- Good, because the app owns the loop.
- Good, because it supports the same platforms as winit.
- Bad, because each build compiles and links a C library.
- Bad, because SDL also offers a GPU API of its own, which overlaps with wgpu.
- Bad, because the wgpu documentation and examples use winit instead.

### GLFW

GLFW is a C library with Rust bindings. It is built for graphics programs on the desktop.

- Good, because its API is small. C and C++ graphics tutorials also use it widely.
- Bad, because each build compiles and links a C library.
- Bad, because it supports desktop platforms alone. A port to a phone would need a different library.
- Bad, because the wgpu documentation and examples use winit instead.

## More Information

Sarx adopted winit 0.30.13 on 18 October 2026, when the developer first recorded this decision. It
was the latest stable release at that time. `Cargo.lock` records the version in use after any later
upgrade.
