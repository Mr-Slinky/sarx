---
status: accepted
date: 2026-11-11
decision-makers: Kheagen Haskins
---

# Telekinetic Hand

## Context and Problem Statement

The god's hand is the centre of Sarx. The player acts on the world through it. The hand picks up
villagers and objects, carries them and throws them. It also strikes things. It can flick a villager
far across the land, or slap the creature.

The hand moves between poses, such as open, fist and pointing. ADR 0014 stores each pose as a fixed
target, which the bones of the hand turn towards. A pose keeps the same shape whatever the hand
holds.

Objects in the world vary widely in shape and size. A villager, a rock, a tree and the creature all
meet the same hand.

Every change that the hand makes to the world arrives as a request. One gate checks each request
against the rules of the level, as ADR 0007 sets out. ADR 0010 moves objects under forces and
resolves their collisions. A thrown object falls under gravity, and comes to rest against the
ground.

How does the hand act on the objects in the world?

## Decision Drivers

- The hand is the centre of the game. Its actions must feel physical. A flicked villager must fly,
  and a slapped creature must reel.
- Each pose is a fixed target, as ADR 0014 sets out. A fist has one shape, whether it holds a
  villager, a rock or a tree.
- The hand never passes through an object. It moves over villages and forests, always above whatever
  lies beneath it.

## Considered Options

- Telekinesis, with collision for strikes
- A physical hand that grasps by contact
- Telekinesis alone

## Decision Outcome

Chosen option: "Telekinesis, with collision for strikes", because telekinesis lets each pose stay
one fixed target whatever the hand holds. Collision during strikes keeps the flick and the slap
physical.

Telekinesis means that the hand moves an object without touching it. To pick up an object, the hand
takes on a lifting pose, and the object rises towards a point beneath the hand. The object then
follows that point while the hand carries it. The telekinesis in Destroy All Humans moves objects in
the same way.

When the hand lets go, the object keeps the velocity that it had at that moment. From then on, it
moves under the physics engine of ADR 0010. An object that the hand flings therefore flies on, falls
under gravity and lands.

The hand has a collision shape at all times, and uses it in two ways.

Outside a strike, the hand hovers above whatever lies beneath it. Where an object such as a tree or
a rooftop would meet the collision shape, the hand rises until it clears that object. As a result,
the hand never touches an object outside a strike, and never passes through one.

A strike is an action such as a flick or a slap. During a strike, the hand stops rising over
objects. The physics engine then pushes away any object that the hand meets.

Each action of the hand combines a pose with an effect on the world. The pose comes from ADR 0014.
The effect, such as lifting a villager or striking the creature, reaches the world as a request
through the gate of ADR 0007.

Five points remain open. The developer settles each one once the hand acts on the world:

- the collision shape of the hand, such as a set of spheres or capsules
- how quickly the hand rises over an object, and how quickly it settles back down beyond it
- how a held object moves towards the point beneath the hand, and whether it collides with the
  world while held
- whether a strike against something heavy, such as the creature, moves the hand itself
- which actions count as strikes, beyond the flick and the slap

### Consequences

- Good, because each pose stays one fixed target. A lifting pose works for a villager, a rock and a
  tree alike, since the fingers of the hand never have to fit round the object.
- Good, because the hand never passes through a tree or a rooftop, and never knocks one over by
  accident. It rides up and over a dense village.
- Good, because a flick and a slap are real collisions. The struck object moves according to where
  the hand hit it, and how fast.
- Good, because a thrown object moves under the same physics as every other object. A throw needs
  only the release.
- Bad, because a contact with the hand has two outcomes. Outside a strike, it raises the hand.
  During a strike, it pushes the object. The code must switch between the two at the start and end
  of each strike.
- Bad, because the height of the hand changes with whatever lies beneath it. Over a village, the
  hand rises over each roof in turn. That rise must be smooth, or the hand jumps.
- Bad, because a held object floats beneath the hand, rather than resting in its palm. The look of a
  lift therefore depends on the pose, and on how the object moves while held, such as a gentle bob.
- Bad, because the collision shape must follow the pose. A flick touches with a fingertip, whereas a
  slap touches with the palm.

### Confirmation

Nothing in the code embodies this decision yet. Once the hand acts on the world, a reader confirms
it with three checks:

- A held object moves towards a point beneath the hand. No code attaches it to a bone of the hand.
- Outside a strike, a contact raises the hand and leaves the object where it stands.
- During a strike, a contact pushes the object.
- Each action of the hand reaches the world as a request through the gate.

## Pros and Cons of the Options

### Telekinesis, with Collision for Strikes

The hand lifts and carries objects without touching them. Outside a strike, it hovers above every
object, and rises to clear each one. During a strike, it collides with objects and pushes them.

- Good, because every pose suits every object, since the hand never closes round one.
- Good, because flicks and slaps are physical.
- Bad, because a contact has two outcomes. Outside a strike it raises the hand, and during a strike
  it pushes the object.

### A Physical Hand that Grasps by Contact

The hand would collide with the world at all times. To pick up an object, its fingers would close
round the object, and contact between the fingers and the object would hold it.

- Good, because a held object would rest in the palm, which looks natural.
- Bad, because a fixed pose fits only one shape. A fist that closes round a villager would sink into
  a tree, and close on air round a pebble. Each kind of object would need a pose of its own.
- Bad, because the hand would bump into rooftops and trees as it moved over a village.

### Telekinesis Alone

The hand would act on the world through telekinesis alone, with no collision shape.

- Good, because the hand would stay out of physics entirely.
- Bad, because the hand would pass through trees and buildings.
- Bad, because a flick or a slap would involve no contact. A scripted effect would move the struck
  object, rather than the hit itself.
- Bad, because the hand would lose the physical feel that makes it the centre of the game.
