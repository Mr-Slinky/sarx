---
status: accepted
date: 2026-10-26
decision-makers: Kheagen Haskins
---

# Simulation Apart from Rendering

## Context and Problem Statement

Sarx repeats two kinds of work while the game runs.

The first is the simulation, which advances the world one step at a time. In each step, the systems
of the ECS update their components. The physics engine also moves objects and resolves collisions.

The second is the renderer. It draws the world as it stands into the window, one frame at a time.

A screen shows a new frame at its refresh rate. A laptop screen commonly refreshes at 60 Hz, and a
gaming monitor on a desktop at 144 Hz or more. A slower GPU also takes longer to draw each frame. On
any machine, the time from one frame to the next varies.

The game loop is the code that ties the two kinds of work together. It decides how many simulation
steps run for each frame, and how much game time each step covers. Those two choices decide whether
the same game plays out the same way on every device.

How does the simulation advance in time, relative to the frames that the renderer draws?

## Decision Drivers

- A player on any device plays the same game. A laptop with a 60 Hz screen and a desktop with a
  144 Hz monitor see the world change at the same rate.
- The simulation has its own crate, `zurvan`, which does not depend on any graphics code.
- The simulation loop is structure. The developer builds it by hand, and extends it as the ECS and
  the physics engine build on it.

## Considered Options

- Fixed steps, apart from rendering
- A variable step, once per frame
- One fixed step per frame

## Decision Outcome

Chosen option: "Fixed steps, apart from rendering", because it is the only option of the three in
which every device runs the same steps at the same rate.

The simulation advances in fixed steps, each covering the same length of game time, and the steps
run at the same rate on every device. The renderer, meanwhile, draws the latest state as often as
the screen allows. As a result, a laptop with a 60 Hz screen plays the same game as a desktop with a
144 Hz monitor.

The loop keeps a running total of the real time that has passed. It adds at most 250 ms to that
total in any one frame, and discards any time beyond that. At the proposed rate of 60 steps per
second, this cap allows 15 steps in one frame.

The cap matters on a slow device. Such a device takes longer to compute a step than the game time
that the step covers. With the cap, it runs the game in slow motion. Without the cap, it would fall
further behind on every frame, until the backlog of steps froze the game.

The simulation lives in the `zurvan` crate, which depends on `enki` and does not depend on any
graphics code. The first fixed-step simulation arrives with the TODO item "Draw the god's hand and
move it with the cursor across a flat ground plane".

Two questions remain open. This decision leaves both to a later discussion.

- 60 steps per second is the proposed step rate. The developer has yet to settle it.
- The renderer can draw the latest state as it stands. It can also blend the two most recent
  states, by how far the clock has run into the next step. That blending is called interpolation.
  The developer has yet to choose between these.

### Consequences

- Good, because every system and the physics engine work with one constant step length. A physics
  step of the same length every time gives the same result from the same starting state.
- Good, because a faster screen draws more frames, while the world moves at the same speed.
- Good, because `zurvan` builds, and its tests run, without a window or a GPU.
- Bad, because the step rate and the frame rate rarely match. A frame can show the same state as the
  frame before it. A frame can also arrive after more than one step has run. Motion then judders.
  Interpolation is the usual remedy, and it remains open.
- Bad, because a slow device cannot keep up with real time. It takes longer to compute a step than
  the game time that the step covers. The 250 ms cap stops the game from freezing on that device,
  and the game runs in slow motion there instead. Such a device falls below the minimum requirements
  of the game.

### Confirmation

`cargo tree -p zurvan` does not list a graphics crate, such as `wgpu`, `winit` or `amaterasu`.
The `[dependencies]` table in `crates/zurvan/Cargo.toml` shows the same.

Once the simulation exists, a reader also checks the code for four points:

- The step length is a single constant.
- The function that advances the simulation by one step does not take elapsed frame time.
- The game loop calls that function as many times as the clock calls for, separately from its call
  to draw a frame.
- The loop adds no more than 250 ms to its running total in one frame.

## Pros and Cons of the Options

### Fixed Steps, Apart from Rendering

The loop measures the real time that has passed, and adds it to a running total. It then runs one
step for each step length that the total contains, and subtracts that time from the total. Last, it
draws a frame.

- Good, because the world advances at the same rate on every device.
- Good, because each step covers the same game time, which makes the physics repeatable.
- Bad, because the loop keeps a running total of time, and decides how many steps to run each frame.
- Bad, because smooth motion between steps needs interpolation in the renderer.

### A Variable Step, Once per Frame

The simulation advances once per frame. Each step covers the real time that has passed since the
previous frame.

- Good, because the world advances at the same rate as real time on every device.
- Good, because the loop is short. It measures the time, advances the world by it, and draws.
- Good, because every frame shows a state that no earlier frame showed.
- Bad, because the same starting state gives different results on different devices. A physics step
  over a longer time moves an object further between collision checks. A fast object on a slow
  device can then pass through a wall that it would hit on a fast device.
- Bad, because every system has to scale its work by the step length. That length changes on every
  frame.

### One Fixed Step per Frame

The simulation advances one fixed step for every frame that the renderer draws.

- Good, because the loop is the shortest of the three. It steps once, then draws once.
- Good, because each step covers the same game time, which makes the physics repeatable.
- Bad, because the game runs at the speed of the screen. A desktop at 144 Hz runs the world 2.4
  times as fast as a laptop at 60 Hz.
- Bad, because a slow frame slows the whole game for as long as it lasts.
