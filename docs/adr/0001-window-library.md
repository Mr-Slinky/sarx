---
status: accepted
date: 2026-10-18
decision-makers: Kheagen Haskins
---

# winit as the Window Library

## Context and Problem Statement

Sarx needs a window to draw into, and an event loop that delivers input and other events from the
operating system to the game. The event loop is the first piece of structure in `sarx-app`, and the
GPU setup and the fixed-step simulation build on it in later steps.

On Android, the operating system controls the lifetime of the app. It can destroy the window while
the app is in the background and hand the app a new one when it returns. The renderer draws into a
surface that belongs to one window, and Sarx therefore rebuilds that surface whenever the window
changes.

Which library, if any, opens the window and runs the event loop?

## Decision Drivers

- The mobile-first design decision requires the window layer to run on Android as well as on the
  desktop.
- wgpu creates its surface from the raw window handles that each platform defines, meaning the window
  layer has to provide them.
- The window layer belongs to the operating system rather than to the graphics pipeline or the engine,
  which are the parts that Sarx sets out to teach.
- Sarx learns from the wgpu documentation and examples, and matching their window layer keeps those
  examples close to Sarx's own code.

## Considered Options

- winit
- A hand-written platform layer
- SDL3
- GLFW

## Decision Outcome

Chosen option: "winit", because it is the only option that provides the window, the event loop and
the Android lifecycle in pure Rust, and the wgpu documentation uses it.

### Consequences

- Good, because one platform layer covers every target that Sarx plans to ship.
- Good, because the examples in the wgpu repository open their windows with the same library.
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
Android, macOS, iOS, Linux and the web through one API.

- Good, because the wgpu documentation and every example in the wgpu repository use it.
- Good, because it covers Android, including the lifecycle events that report a lost or a new window.
- Good, because it hides only operating-system plumbing.
- Neutral, because its architecture follows from the platforms themselves. Android and the web call
  into an app when an event arrives, meaning a library for them cannot let the app own the loop.
- Bad, because Sarx implements a trait, and winit calls it for each event.
- Bad, because its API changes between major releases.

### A Hand-Written Platform Layer

Sarx would call Win32 directly on Windows. On Android, it would use the NDK, Android's toolkit for
native code, together with JNI, the bridge between native code and Java. Sarx would then pass the raw
window handles to wgpu itself.

- Good, because it teaches how each operating system delivers input and runs an app.
- Good, because Sarx owns every line, with no outside design to follow.
- Bad, because that knowledge belongs to the operating system rather than to the graphics pipeline or
  the engine.
- Bad, because each platform needs a separate version.

### SDL3

SDL3 is a C library with Rust bindings. The app runs its own loop and polls SDL for events, which
matches the classic shape of a game loop.

- Good, because the app owns the loop.
- Good, because it supports Android.
- Bad, because each build compiles and links a C library, Android included.
- Bad, because it overlaps with wgpu, since SDL also offers its own GPU API.
- Bad, because the wgpu documentation and examples use winit instead.

### GLFW

- Good, because its API is small and well known from C and C++ graphics tutorials.
- Bad, because it supports the desktop alone, which the mobile-first decision rules out.

## More Information

Sarx adopts winit 0.30.13, the latest stable release on the date of this decision.
