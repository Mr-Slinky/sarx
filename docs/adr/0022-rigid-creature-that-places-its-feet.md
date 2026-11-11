---
status: accepted
date: 2026-11-11
decision-makers: Kheagen Haskins
---

# A Rigid Creature that Places Its Feet

## Context and Problem Statement

The creature is one of three parts of Sarx that the developer means to stand out, alongside the
hand and combat. The player trains the creature, and can possess it during a battle.

ADR 0012 gives Sarx a stylised look, and builds each villager from rigid parts. It leaves the
building and animation of the creature to a separate decision. This ADR records that decision.

Black & White 2 gave its creatures the forms of real animals, such as a lion and a cow, covered in
fur. The developer builds models in code and through Blender scripts, and has little experience of
sculpting.

The creature walks through a world that the player has built. The player lays roads and paths.
Villages pack their buildings close together. The land is a heightmap, as ADR 0004 sets out, which
means that the ground beneath each foot can lie at a different height.

How does Sarx build the creature, and how does the creature move through the world?

## Decision Drivers

- The creature follows the stylised look of ADR 0012.
- The developer builds models in code and through Blender scripts, rather than by sculpting.
- The creature must move through the world sensibly. It must follow the roads and paths that the
  player builds. It must pass through a dense group of buildings, such as a cluster of hovels,
  without passing through any building.
- Each foot must land on the ground. On uneven land, a foot that floats above the ground or sinks
  into it breaks the sense of weight.
- The creature must feel heavy.

## Considered Options

- Rigid parts, with legs that place each foot
- Rigid parts, with a recorded walk cycle
- A skinned, realistic animal, as in Black & White 2

## Decision Outcome

Chosen option: "Rigid parts, with legs that place each foot", because a creature that chooses where
each foot lands can walk on uneven land, follow a road and step between buildings.

The creature is built from rigid parts, as a villager is. Each leg is a chain of rigid segments,
joined end to end. The creature never has fur or hair.

The shape of the creature remains open, and two directions fit this decision. In the first, code
generates the shape from rules and numbers, which is called procedural generation. In the second,
the developer designs a mechanical creature from simple parts, perhaps shaped like an insect.

The creature walks by procedural animation, rather than by playing back a recorded walk. Procedural
animation means that code works out each movement as it happens. On each update, the code chooses
where each foot lands next, on the ground ahead and clear of every building. The code then turns
the joints of the leg until the foot reaches that point. Working out the joint angles from the point
that the end of a chain must reach is called inverse kinematics.

The creature plans its route before it walks. The route prefers the roads and paths that the player
built, and keeps clear of every building.

The developer builds the creature late in the work, after the hand and the villagers.

Five points remain open. The developer settles each one when the creature is built:

- the shape of the creature, and whether code generates it
- the number of legs, and the order in which they step, which is called the gait
- how the creature finds its route, and the map of roads and buildings that it plans over
- how each step moves the body, such as how far it sinks and sways, to give the creature weight
- how the trained AI of the creature chooses where the creature goes

### Consequences

- Good, because each foot lands on the ground at whatever height the heightmap gives there.
- Good, because the creature can walk any route, including a bending road or a gap between two
  hovels. A recorded walk covers only the moves that someone recorded.
- Good, because the same code walks the creature while the player possesses it. The player steers,
  and the code still places each foot.
- Good, because the renderer draws the creature in the same way as a villager, with one transform
  per part. Rigid parts do not need skinning.
- Good, because the renderer never has to draw fur or hair.
- Bad, because the look of the walk comes from code rather than from an animator. The developer
  tunes the weight and timing of each step through numbers.
- Bad, because the creature needs a route planner that knows where every road and building lies.
  The planner must also fit the whole body of the creature through each gap.
- Bad, because each leg is a hierarchy of parts. ADR 0012 notes that a hierarchy needs a design of
  its own within the component storage of the ECS.
- Bad, because the creature can never take a furred form, such as the lion of Black & White 2. That
  rules out a whole family of animal designs.

### Confirmation

Nothing in the code embodies this decision yet. Once the creature walks, a reader confirms it with
four checks:

- The creature is built from separate rigid meshes, with no skin weights.
- No model or shader draws fur or hair on the creature.
- No code plays a recorded walk cycle for the creature. Each foot target comes from the ground
  ahead of it.
- The route of the creature never crosses a building.

## Pros and Cons of the Options

### Rigid Parts, with Legs that Place Each Foot

Code chooses where each foot lands, then turns the joints of the leg to reach that point.

- Good, because every foot meets the ground, on any terrain and along any route.
- Good, because the creature can walk wherever its route planner sends it.
- Bad, because the walk is only as convincing as its tuning. A recorded walk shows exactly what the
  animator drew.

### Rigid Parts, with a Recorded Walk Cycle

A walk cycle is a short clip of one full stride. The clip plays on a loop while the creature moves.

- Good, because an animator would control every frame of the stride.
- Bad, because the clip places each foot at a fixed height. On a slope, the feet would float above
  the ground or sink into it.
- Bad, because the feet would slide across the ground whenever the creature moved faster or slower
  than the clip assumes.
- Bad, because the clip would put each foot wherever it was recorded. Stepping between hovels would
  need further clips, and a foot could still land inside a wall.

### A Skinned, Realistic Animal, as in Black & White 2

The creature would be one mesh wrapped round a skeleton, shaped like a real animal and covered in
fur.

- Good, because it would match the creatures that players remember from Black & White 2.
- Bad, because a realistic animal would stand apart from the stylised look of ADR 0012.
- Bad, because a realistic animal needs the skill of a sculptor, which code and Blender scripts fall
  short of.
- Bad, because a furred animal needs the renderer to draw fur.
