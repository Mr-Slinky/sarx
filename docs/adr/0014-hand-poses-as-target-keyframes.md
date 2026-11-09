---
status: accepted
date: 2026-11-05
decision-makers: Kheagen Haskins
---

# Hand Poses as Target Keyframes

## Context and Problem Statement

The god's hand is a skinned mesh. A skinned mesh is one mesh that wraps round a skeleton of bones.
Each vertex follows up to four of those bones, and blends their movements by weight. ADR 0012 leaves
the animation of the hand to a separate decision. This ADR records that decision.

The hand moves between poses, such as open, fist and pointing. A pose gives each bone of the hand a
rotation from its rest position. The pose that the hand moves towards is its goal. The player can
change that goal at any moment. As a result, the hand can be half-way into a fist when it has to
start pointing instead.

The hand comes from Blender, through scripts that pose its bones. The hand reaches Sarx as a glTF
file, which can also store animations. A glTF file stores an animation as keyframes. A keyframe
gives a bone its rotation at a point in time.

How does Sarx store the hand's poses, and how does the hand move between them?

## Decision Drivers

- The hand changes its goal at any moment, including part-way through a move. Its movement must stay
  smooth through each change.
- The developer builds and checks each pose in Blender, where the pose appears on the mesh.
- The poses must match the skeleton that they move. A bone renamed in Blender must not leave a pose
  pointing at a bone that no longer exists.
- Sarx animates the hand procedurally, in its own code, rather than playing back a recorded clip.

## Considered Options

- Each pose as a one-keyframe animation in the hand's glTF file
- Each pose in a separate pose file
- Recorded animations from one pose to another, played back

## Decision Outcome

Chosen option: "Each pose as a one-keyframe animation in the hand's glTF file", because the hand's
glTF file stores the poses beside the skeleton that they move, and Blender exports them with no
extra tooling.

Each pose is an animation with one keyframe. The animation takes the name of the pose, such as
`fist`. The keyframe gives each bone of the hand a rotation. The wrist and forearm bones belong to
the arm, which means that a hand pose leaves them out.

Sarx reads each pose as a target, and never plays it as a clip. On each update, every bone turns
from its current rotation towards its rotation in the target pose. The turn uses slerp, short for
spherical linear interpolation. Slerp finds the rotation that lies a given fraction of the way from
one rotation to another, along the shortest arc between them.

To change the hand's goal, Sarx changes the target pose. Each bone then starts its turn from its
rotation at that moment. A hand part-way into one pose therefore turns smoothly towards the next.

ADR 0015 sets how far a bone turns on each update. A further decision must choose whether the
update runs in the simulation step or in the renderer, and that decision must agree with ADR 0005.

### Consequences

- Good, because Sarx handles a change of goal at any moment like any other update. Each bone always
  turns from its current rotation.
- Good, because a pose stores little data. It stores one rotation for each of about twenty bones.
- Good, because the developer sees and adjusts each pose in Blender, on the real mesh.
- Good, because different bones can follow different poses. The thumb could follow `fist` while the
  index finger follows `point`.
- Bad, because the shader has to skin the hand. For each vertex, the shader blends up to four bone
  transforms by weight. Villagers do not need skinning, because ADR 0012 builds them from rigid
  parts.
- Bad, because Sarx has to read the animations section of a glTF file, as well as its meshes and its
  skin. The choice between the `gltf` crate and a hand-written parser applies to this section as
  well.
- Bad, because a pose stores only where the hand ends up. A move along a set path, such as a wave,
  needs more than one keyframe. It also needs a different way to play the move back.

### Confirmation

Nothing in the code embodies this decision yet. Once the hand animates, a reader confirms it with
three checks:

- Each pose in the hand's glTF file is an animation with a single keyframe.
- The animation code turns each bone towards a target rotation by slerp, starting from its current
  rotation.
- No code plays a hand animation through time.

## Pros and Cons of the Options

### Each Pose as a One-Keyframe Animation in the Hand's glTF File

Blender stores each pose as a named action with one keyframe. The glTF exporter in Blender then
writes each action as an animation.

- Good, because one file stores both the poses and the skeleton. The two therefore stay in step.
- Good, because Blender exports the poses with no extra tooling.
- Bad, because Sarx has to read glTF animations. A loader that reads only meshes would skip that
  part of the file.

### Each Pose in a Separate Pose File

A Blender script would read the rotation of each bone. It would then write the rotations to a small
file, in a format that Sarx defines.

- Good, because the developer would define the format, and its parser would be short.
- Bad, because the pose file and the glTF file can fall out of step. A bone renamed in Blender would
  break the pose file silently.
- Bad, because every pose would need a run of the script, in addition to the glTF export.

### Recorded Animations from One Pose to Another, Played Back

The developer would record a clip in Blender for each move, such as open to fist. Sarx would then
play the clip through time.

- Good, because an animator would control every frame of the move.
- Bad, because each pair of poses would need a clip of its own. The number of clips would then grow
  with the square of the pose count.
- Bad, because a clip always starts from one fixed pose. Where the goal changes part-way through a
  clip, Sarx would have to blend two clips, or wait for the first to finish.
