---
status: accepted
date: 2026-11-11
decision-makers: Kheagen Haskins
---

# Stylised Art

## Context and Problem Statement

Sarx puts thousands of villagers on screen at once. A creature and the god's hand appear alongside
them. Every one of these needs a model, and most of them need animation.

The developer writes code, and has little experience of modelling or animating by hand. The models
therefore come from three sources:

- models built from simple shapes in code
- scripts that build models in Blender
- AI generators that turn a description or a sketch into a model file

The appeal of the game comes from its mechanics. The creature learns, the civilisation advances on
its own, and the factions respond to the player.

ADR 0003 also keeps Sarx within the GPU limits that a mid-level laptop meets.

Black & White 2 is the game that Sarx follows for the creature and the hand. It drew realistic
people and animals with few triangles. As a result, their hands and faces looked blocky.

Players judge a game against other games in the same style. A realistic third-person game stands
beside Red Dead Redemption 2, Assassin's Creed and Grand Theft Auto. Large studios built each of
those over many years. A stylised game stands beside Astro Bot, Crash Bandicoot and Spyro instead.
Those games win players over with timing, feedback and playful mechanics, rather than with
realistic detail.

Sarx meets that comparison directly. During a battle, the player can possess a character and
control it in third person, as the Planned Features in the README set out.

What does Sarx look like, and how does it build its characters?

## Decision Drivers

- The mechanics decide whether Sarx is a good game. The look serves them.
- The look decides which games players compare Sarx to. A single developer must be able to match
  the best of those games.
- Sarx runs within the GPU limits that a mid-level laptop meets, as ADR 0003 sets out.
- Models come from code, from Blender scripts and from AI generators. The look therefore has to suit
  simple geometry. It also has to suit the uneven triangles that a generator produces.
- Thousands of villagers move at once. Their animation has to take less work the further they are
  from the camera.

## Considered Options

- Stylised, with characters built from rigid parts
- Low-poly realism
- Realism

## Decision Outcome

Chosen option: "Stylised, with characters built from rigid parts", because the look is made of
simple, clean shapes, and every source of models that Sarx has produces shapes of that kind. The
look also places Sarx beside Astro Bot and Crash Bandicoot, whose strengths a single developer can
match.

Sarx looks stylised, in the manner of Astro Bot, Universim and Minecraft. It uses bright colours,
clean shapes and simple forms. Its proportions and surfaces are simplified rather than realistic.

Each villager is built from separate rigid parts, as the characters of Astro Bot are. A head, a
body, two hands and two feet are each a small mesh of their own. No joints connect them.

Animating a villager moves and rotates whole parts. Each part therefore needs only its own
transform. A transform is the matrix that places the part in the world. Nothing bends, and no vertex
blends between bones.

Combat follows the same look, and is comic rather than grounded. A blow can knock the head off a
villager, or tear away a limb, and the part flies off across the land.

The creature and the hand follow the same look. How the developer builds and animates them is a
separate decision.

### Consequences

- Good, because the animation of a villager is one transform per part. The renderer sends the
  matrix of each part to the shader, with no skin weights and no skeleton to blend.
- Good, because villagers far from the camera can take less work. Near villagers animate every part
  on every frame. Villagers further off update less often. Distant villagers can merge their parts
  into one mesh.
- Good, because a stylised look suits simple geometry. Models built in code can then stand beside
  models from an AI generator.
- Good, because simple models and plain surfaces keep the GPU work within what a mid-level laptop
  can do.
- Good, because a villager loses a part in combat by letting go of it. The part becomes an object of
  its own, which the physics engine of ADR 0010 then throws. A rigid part has no seam to hide where
  it joined the body. Knocking off a head therefore does not need extra modelling or extra animation.
- Bad, because a figure built from parts is a hierarchy. The position of a hand depends on the
  position of the body. The position of the body depends in turn on the position of the villager.
  ADR 0011 settles how the ECS stores components, and a hierarchy needs a design of its own within
  that storage.
- Bad, because models from several sources have to match one another in colour and proportion. A
  generated model that ignores the palette stands out among the rest.

### Confirmation

Nothing in the code embodies this decision yet. Once villagers exist, a reader confirms it with two
checks:

- Each villager is built from separate meshes, each with its own transform.
- The shaders do not blend vertices between bones.

## Pros and Cons of the Options

### Stylised, with Characters Built from Rigid Parts

The world uses bright colours and simple forms. Each character is a set of separate rigid meshes.
Astro Bot builds its characters this way.

- Good, because simple shapes are part of the look. Code and AI generators both produce simple
  shapes.
- Good, because animating rigid parts needs one transform per part. That is little work, even for
  thousands of villagers.
- Good, because it places Sarx beside Astro Bot and Crash Bandicoot. Comic, playful combat suits
  those games, and the parts of a figure can fly apart in a fight.
- Bad, because the parts form a hierarchy, which the ECS has to store.

### Low-Poly Realism

The world draws realistic people, animals and landscapes with few triangles. Black & White 2 looks
this way.

- Good, because it matches the look of the game that Sarx follows for the creature and the hand.
- Bad, because realistic characters bend at their joints. Bending needs skinned meshes. A skinned
  mesh is one mesh wrapped round a skeleton, and each of its vertices blends between the bones near
  it.
- Bad, because a realistic shape with few triangles shows blocky hands and faces. A stylised shape
  avoids this, since it never aims at realism.

### Realism

The world aims at realistic models, materials and lighting.

- Good, because it can produce the most striking screenshots.
- Bad, because realistic models and textures need the skill of an artist. Code and AI generators
  fall short of that skill.
- Bad, because realistic lighting and detailed models need more GPU power than a mid-level laptop
  has.
- Bad, because a realistic third-person game stands beside the best-funded games ever made.
  Possession puts Sarx in third person during battle, where combat animation would face that
  comparison.
