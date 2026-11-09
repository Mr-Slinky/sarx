---
status: accepted
date: 2026-11-05
decision-makers: Kheagen Haskins
---

# Hand Bone Turns by Critically Damped Spring

## Context and Problem Statement

Under ADR 0014, each pose of the hand is a target. On every update, each bone of the hand turns from
its current rotation towards its rotation in the target pose. The player can change the target at any
moment, including part-way through a move.

ADR 0014 leaves one question about that turn to a separate decision. The question is how far a bone
turns on each update. The answer sets how a move looks. It decides how a move starts, how it ends,
and how the hand behaves when the target changes part-way through.

How far does each bone of the hand turn on each update?

## Decision Drivers

- A move must look like the move of a real hand. A real hand gathers speed at the start of a move,
  and slows down as it reaches the pose.
- A change of target part-way through a move must leave the hand's speed smooth. A sudden jump in
  speed looks like a twitch.
- A bone must not swing past its pose and back.
- One setting must control how quick the hand feels.
- The update must stay steady whatever time an update spans. ADR 0014 leaves open whether the update
  runs in the simulation step or in the renderer.

## Considered Options

- A fixed fraction of the remaining angle on each update
- A fixed angle on each update
- A timed easing curve for each move
- A critically damped spring

## Decision Outcome

Chosen option: "A critically damped spring", because it is the one option that eases into a move as
well as out of it, and it keeps the hand's speed smooth through a change of target.

Each bone of the hand must keep two values while the game runs. The first is its rotation, and the
second is its angular velocity. The angular velocity is a vector of three floats. Its direction is
the axis that the bone turns about, and its length is the bone's turning speed. A hand of 20 bones
therefore needs 60 floats in addition to its rotations.

On each update, Sarx must work out the offset of each bone from its target. The offset is the turn
that takes the target rotation to the current one. Sarx must store it as a vector of the same kind as
the angular velocity. A spring then pulls the offset towards zero, while damping slows the bone's
turn. Critical damping is the amount of damping that brings the bone to its target in the least time
without swinging past it.

The update must use the exact solution for a critically damped spring, taken over the whole time
that the update spans, rather than adding up small steps. Small steps can swing back and forth, or
grow without limit, when an update spans a long time. The exact solution stays steady for any update
time.

The update works with four values:

- `omega` is the spring's one setting.
- `dt` is the time that the update spans.
- `offset` is the offset vector.
- `velocity` is the angular velocity.

On each update, Sarx must run the steps below for each bone.

```text
decay    = exp(-omega * dt)
pull     = (velocity + omega * offset) * dt
velocity = (velocity - omega * pull) * decay
offset   = (offset + pull) * decay
rotation = rotation_from_vector(offset) * target
```

A higher `omega` brings the bone to its target sooner. The developer must tune it by eye.

When the target changes, Sarx must recalculate the offset from the new target. It must leave the
angular velocity as it is. The bone therefore keeps its speed as it starts to turn towards the new
pose.

### Consequences

- Good, because a move gathers speed over its first few updates, and slows down into the pose.
- Good, because the hand's speed stays smooth through a change of target.
- Good, because no bone swings past its pose.
- Good, because one setting, `omega`, controls how quick the hand feels.
- Good, because the exact solution stays steady for any update time. The update therefore works in
  the simulation step and in the renderer alike, whichever the later decision picks.
- Bad, because each bone must keep an angular velocity as well as its rotation.
- Bad, because each update needs more arithmetic than a single slerp. The update must convert a
  rotation to a vector and back, and must compute an exponential.
- Bad, because a move starts more slowly than under a fixed fraction. As a result, the hand responds
  to a new target a few updates later than under a fixed fraction.

### Confirmation

The code does not embody this decision yet. Once the hand animates, a reader confirms it with
three checks:

- Each bone of the hand keeps an angular velocity of three floats beside its rotation.
- The update moves each bone by the exact critically damped solution above, with `omega` as its only
  setting.
- A change of target leaves each bone's angular velocity as it was.

## Pros and Cons of the Options

The developer compared the spring against the fixed fraction in a Blender preview. The preview moved
the hand through the saved poses one update per frame, and changed the target once part-way through
a move. The figures below come from that preview.

### A Fixed Fraction of the Remaining Angle on Each Update

On each update, a bone would turn by a set share, such as 15 percent, of the angle that remains to
its target.

- Good, because it needs only the bone's rotation, and one slerp per update.
- Good, because a move slows down as it reaches the pose.
- Bad, because a move starts at its full speed on the first update. In the preview, the index knuckle
  turned 12.8 degrees on the first update of a fist, against 3.1 degrees under the spring.
- Bad, because a change of target makes the speed jump. In the preview, a bone's speed rose by up to
  18 degrees in one update, against 4 degrees under the spring.
- Bad, because a bone never lands on its target, since each update covers only part of what remains.

### A Fixed Angle on Each Update

On each update, a bone would turn by at most a set angle, such as 6 degrees.

- Good, because it needs only the bone's rotation, and one slerp per update.
- Good, because a bone lands exactly on its target.
- Bad, because a move runs at one even speed, and stops at once at the pose. The motion looks
  mechanical.

### A Timed Easing Curve for Each Move

Each move would take a set time, and follow an easing curve from its start rotation to its target.

- Good, because the developer controls how long a move takes, and how its speed changes along the
  way.
- Good, because a bone lands exactly on its target.
- Bad, because a change of target restarts the curve from the bone's current rotation. The new
  curve starts from its own beginning, so the speed jumps.
- Bad, because each bone keeps its start rotation and the time since its move began.

### A Critically Damped Spring

Each bone keeps an angular velocity. A damped spring pulls the bone towards its target, as the
Decision Outcome sets out.

- Good, because a move eases in and eases out.
- Good, because the speed stays smooth through a change of target.
- Bad, because each bone keeps three more floats, and each update needs more arithmetic.
