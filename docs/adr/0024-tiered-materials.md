---
status: accepted
date: 2026-11-14
decision-makers: Kheagen Haskins
---

# Tiered Materials

## Context and Problem Statement

The README sets out how a level ends. A player wins once their faction is the last one standing,
either by allying with every other faction or by conquering them all. That is the win condition. It
leaves open what the player does from the start of a level until then.

A game loop is the cycle of actions that a player repeats throughout a level. Each pass through the
loop leaves the player a little stronger, and gives them a reason to start the next one. Sarx needs
a game loop that carries the player towards the win condition.

ADR 0012 gives Sarx a stylised look, with bright colours and simple forms, and builds each villager
from separate rigid parts. It also leaves open what makes that look belong to Sarx alone.

The first reference images that the developer generated from ADR 0012 came back as generic vinyl
toys: rounded, smooth and cheerful, with a large face. An image model produces that figure whenever
a prompt asks for "cute" and "simple" and nothing more, since it is the average of every mascot that
the model has seen. Players forget a look that any studio could produce from the same words.

The camera also constrains the look. The god's camera mostly looks down on villagers from above,
hundreds at a time. During a battle, the player can possess a character and see it from behind in
third person, as the Planned Features in the README set out. A villager must therefore be easy to
recognise from above and from behind, and the face matters less than the top of the head, the
shoulders and the back.

Spore lets the player build each creature in an editor. An editor of that kind would take the
developer a year or more to build. Every body that it allowed would also need its own walk, its own
collision shape and its own way of breaking apart in combat.

What game loop carries the player towards victory, and can the same idea give the people of Sarx a
look of their own?

## Decision Drivers

- The game needs a loop that the player repeats between the start of a level and victory.
- Each pass through the loop must leave the player with something better to aim for.
- The look must belong to Sarx, rather than to the average of every stylised game.
- The idea behind the look must reach beyond the model, into combat, physics, sound and the story.
  A rule that shapes only the villager gives a style to that one asset alone.
- A villager must be easy to recognise from above, from behind and close up.
- Every source of models that ADR 0012 lists must be able to produce the look: code, Blender
  scripts and AI generators.
- The developer must build one complete slice of the game before widening it.

## Considered Options

- Tiered materials, with people shaped from them
- Tiered materials, with one look for every villager, as ADR 0012 stands
- A body editor, as in Spore

## Decision Outcome

Chosen option: "Tiered materials, with people shaped from them", because one idea, the material,
drives the game loop and decides the look, the combat, the physics and the story together.

### The Game Loop

The player gathers materials, makes things from them, and uses what they make to reach better
materials. Every faction depends on materials throughout a level. It needs them to put up
buildings, to make new people, to prepare for war, and to create and grow its creature.

The materials come in tiers, and a better material makes a better thing. People of a higher tier
are stronger and live longer. A creature of a higher tier is tougher. Buildings of a higher tier are
more beautiful, and stand up better to a siege. Every player therefore works towards the best
materials that they can reach, for everything that they make.

Minecraft uses the same loop, in which a player moves from a wooden pickaxe through to a diamond
one.

### The Tiers

The tiers run between materials, as from clay to bronze, and also within one material, as from a
common wood to a fine one.

The first materials, such as wood, clay and stone, come from history. Later materials can leave
history behind, and move into fantasy and science fiction, as the developer finds ideas that suit
the game.

A better material is also harder to acquire. Making a thing from it needs materials from the tiers
below, more work and more time. A lower tier therefore stays a real choice. Clay is cheap, and a
player can fight a war on numbers rather than on technology, by sending twice as many clay people
against a wooden enemy.

### People Shaped from a Material

The game loop also decides the look. Each person is a small figure shaped from one material, in the
manner of a carved idol or a figurine. The material shows on every rigid part of the figure, and
the tier of that material sets how strong the person is and how long they live.

Each material has its own surface. Clay is slightly jagged and rough, with flat facets and
thumbprints. A metal such as bronze has a smoother, polished surface. Every figure keeps the
proportions that make it cute, such as a large head on a short body. The cute look therefore comes
from the proportions, rather than from rounding every surface.

The material also decides what happens when a figure breaks. Combat is comic, and a blow can knock
off a head or a limb, as ADR 0012 sets out. The break shows the material, thereby giving Sarx a faux
gore of its own. The table below sets out how each material breaks when struck:

| Material | When struck                           |
|----------|---------------------------------------|
| Clay     | cracks, and sheds chunks and dust     |
| Wood     | splinters, and the head pops off      |
| Bronze   | dents, and the part rings as it flies |
| Iron     | throws sparks, and scatters bolts     |

The material can also set the sound of a blow, and the weight and bounce of a part that the hand
throws. A clay head would land heavily and crumble, whereas a wooden one would bounce.

### The Creation Ceremony

A level opens with a ceremony, in which the player shapes their first people. At the start, the game
offers a limited choice of materials, and the player chooses one of them. That choice alone decides
the figures, since the player has no control over the body itself.

The ceremony draws on the myth of Enki, the Sumerian god who shaped the first people out of clay. The
ECS crate, `enki`, already takes his name. Historians name the early ages after materials as well:
the Stone, Bronze and Iron Ages. Hesiod's Ages of Man tells of races made of gold, silver, bronze
and iron. A player therefore meets a familiar idea: the people are made of the material of their
age.

### Reproduction

The people are made rather than born. They shape new figures, in the way that the player shaped the
first ones. A new person uses up the material that the new person is made of, whatever their makers
are made of. Clay people making a bronze person therefore use up bronze.

A village grows in the way that a production line does. To make a person of a given material, the
people need two things: the knowledge of how to shape that material, and the means. The means are
the materials themselves, which the game loop provides. How the people gain the knowledge is an Open
Point, below. A village with both can make as many people as its materials allow.

In the lore, the people have no sex. As a result, a model of a person does not need genitals, and
nudity is normal among the people. A figure wears clothing only where the design calls for it to
mark a role or a faction, rather than for modesty.

### A New Age

A person keeps the material that they are made of for the whole of their life. Reaching a better
material lets a village make new people from it, and leaves its living people as they are.

The player decides what each new person is made of. A village replaces one material with another
only where the player chooses to play that way. A player can instead make only a few people of a
high tier, and keep a small elite above a majority of clay. A player can even try the reverse, a
few people of a low tier among many of a high one, although the cost of the higher tiers makes that
harder to keep up.

### Breaking Down a Person

A person can be broken down into the material that they are made of. The material returns to the
faction that breaks them down, for making new things. A faction can therefore feed its own growth
from an enemy population, by breaking down the people that it defeats.

### The God's Hand and the Creature

The creature is made of a material, as the people are. In the lore, the people build their creature
in the same way that they shape themselves, and the tier of its material sets how tough it is. The
creature begins in the material that the player chooses at the ceremony, as the first people do.

The god's hand, however, is not made of a material. It looks like a normal human hand.

### The First Material

Clay is the first material that the developer will build, as a full vertical slice of the game
around clay figures: the model, the break, the physics and the ceremony. The first people and the
first creature in that slice are therefore clay. Wood and the
other materials will follow once clay has proved the idea.

### Open Points

Five points remain open. The developer will settle each one as the work reaches it:

- how the player reaches each further material, beyond the first limited choice
- how the people gain the knowledge to shape each material, such as by reaching a population count,
  which will take an ADR of its own
- the full list of materials, its order, and where it leaves history for fantasy and science fiction
- the style of the god's hand
- whether possession uses a third-person view, or a first-person view that draws only the two
  floating hands

### Consequences

- Good, because the loop gives the player a goal at every point between the start of a level and
  victory. A better material always lies ahead.
- Good, because every part of the game draws on one supply of materials: the buildings, the people,
  the war and the creature.
- Good, because one value, the material, decides the model, the break, the sound and the weight.
  The idea therefore reaches every part of the game that involves a person.
- Good, because a village must gather its own materials to grow, which ties the population to the
  land around it.
- Good, because a village shows its progress in its own people. Figures of every material that the
  player chose to make live side by side, and the mix tells the story of how the player played.
- Good, because breaking down a person ties combat to the economy. A defeated enemy becomes material
  for the faction that defeated it.
- Good, because the cost of the higher tiers gives a lower tier a place in war. A large army of clay
  can stand against a small army of a better material.
- Good, because a battle shows each side's tier at a glance. Bronze figures fighting clay ones look
  and sound unlike clay fighting clay.
- Good, because the people have no sex. Every model of a person leaves out genitals, and a figure
  can stand without clothing.
- Good, because the ceremony gives the player a moment of creation without a body editor. The bodies
  stay a fixed set of rigid parts, which the walk, the collision shapes and the break all rely on.
- Good, because flat facets suit every source of models. Code and Blender scripts build flat faces
  easily, and the uneven triangles that an AI generator produces look like deliberate facets on clay.
- Good, because rigid parts suit the faux gore. A clay or wooden part breaks off cleanly, with no
  seam to hide, as ADR 0012 already notes.
- Bad, because the developer must balance every tier against the others, for the people, the
  buildings and the creature alike.
- Bad, because each material needs its own surface, its own break effect and its own sounds. Every
  new material adds art and effects work across the whole game.
- Bad, because models from several sources must still match one another within each material. A
  generated clay figure that ignores the shared colours and facets stands out among the rest.
- Bad, because the simulation must track each material as a resource, and a village that runs out
  of a material cannot make people from it. The planner of ADR 0006 must weigh that supply when it
  decides what a village gathers.
- Bad, because the ceremony is a sequence of its own, outside a normal level. The developer must
  build it, and the game must play it before the first village exists.

### Confirmation

Nothing in the code embodies this decision yet. Once villagers exist, a reader confirms it with
eight checks:

- Buildings, new people and the creature each use up materials when a faction makes them.
- A thing made from a material of a higher tier is stronger than the same thing made from a
  lower one.
- Each villager stores its material as data, and the renderer, the physics and the break effects
  read that one value.
- A villager breaks in the way that its material sets out, rather than in one shared effect.
- A level opens with the ceremony, and the material chosen there decides the first people.
- A living person keeps their material when their village reaches a better one.
- Making a new person uses up the material of the new person, rather than that of their makers.
- Breaking down a person returns their material to the faction that breaks them down.

## Pros and Cons of the Options

### Tiered Materials, with People Shaped from Them

The player gathers better materials to make better things, and every person shows the material
that they are made of in their surface, their break and their sound.

- Good, because one idea drives the game loop and the look together, and reaches into combat,
  physics and the story.
- Good, because the player takes part in creating their people, while the bodies stay a fixed set of
  rigid parts.
- Bad, because each material multiplies the art and effects work.

### Tiered Materials, with One Look for Every Villager, as ADR 0012 Stands

The player would gather the same tiered materials, but every villager would share one stylised
look, with bright colours and simple forms.

- Good, because one look needs one set of models and one set of effects.
- Bad, because the first generated images show what the look becomes without further direction:
  the generic vinyl toy that any studio could produce.
- Bad, because the player's progress would show in the buildings and the numbers, but never in the
  people themselves.

### A Body Editor, as in Spore

The player would build the body of their first people in an editor, part by part.

- Good, because each player's people would look unlike anyone else's.
- Bad, because the editor would take the developer a year or more to build.
- Bad, because every body that the editor allowed would need its own walk, its own collision shape
  and its own way of breaking apart in combat.
