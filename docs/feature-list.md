# Planned Features

The features below describe the game that the developer is working towards. Each one is still a
plan.

- The player acts on the world through the **god's hand**, the centre of the game. The hand lifts
  villagers and objects by telekinesis, as in Destroy All Humans, and moves between poses as it does
  so. It hovers above whatever lies beneath it, and also strikes, flicking a villager far across the
  land or slapping the creature. [ADR 0022](adr/0022-telekinetic-hand.md) records how the hand acts
  on objects.
- The player trains a **creature**, teaching it by rewarding and punishing it, as in Black & White
  2. Over time, the creature's behaviour leans towards good or evil. The developer means its AI to go
  well beyond what Black & White 2 offered.

  [ADR 0023](adr/0023-rigid-creature-that-places-its-feet.md) sets out how the creature is built and
  how it walks.
- **Combat** is comic and playful, as in Astro Bot and Crash Bandicoot. A blow can knock the head
  off a villager or tear away a limb, and the part flies off on its own.
- The player's **civilisation** advances on its own, as in Universim and Spore. Each village moves
  through new technologies and stages without the player directing every step.
- Up to three **rival factions**, each run by the computer, compete with the player for the same
  world. Each faction can become an ally as well as an enemy.
- A level ends in **victory** once the player has defeated every rival faction on the map. The
  player defeats a faction in one of two ways. They either build a city impressive enough that the
  faction's people immigrate to it, or they besiege the faction. Each faction can fall a different
  way, so a player can besiege one faction and win over another.

  Some factions are harder to win over than others, and some are harder to besiege. The factions
  also respond to the path that the player takes. They might ally with one another against a
  conqueror. They might also turn hostile over whom the player has allied with. Either way, each
  path grows harder the further the player takes it.
- A player starts a **level** either by configuring one, with settings such as the number of
  factions and the kind of landscape, or by picking a pre-made level.
- During a battle, the player can take direct control of a single character, through
  **possession**. Rise & Fall: Civilizations at War works the same way, letting the player command a
  hero such as Alexander the Great. The creature is the first character that the player can possess,
  and very likely the only one. Possessing villagers or soldiers remains an option for later.
- The player casts **miracles and spells**. The magic draws on both Universim and Black & White 2.
- The world offers **cosy play**, gentle activities such as farming and fishing. A player can spend
  a whole session on these, tending the world.
- A player can switch on **cosy mode**. In cosy mode, no faction attacks the player, and the player
  does not attack anyone. The player is then free to manage their civilisation in peace.

  A player in cosy mode cannot complete the level. Three things switch cosy mode off: a hostile
  action, a trade, and any approach to a possible ally.

  The developer must also stop a player from exploiting cosy mode. A player could, for example, stay
  in cosy mode until their civilisation is large. Two approaches remain open:
  - The player would choose cosy mode before a level starts, and it would stay fixed for that level.
  - Cosy mode would allow only the actions that keep the civilisation going. It would not allow new
    buildings or population growth.

[docs/planned-systems.md](planned-systems.md) lists the systems that these features would need, and
how each system would use the tables of the ECS. The developer and every agent read that list before
planning a feature or designing a part of the ECS, and add the systems of each new feature to it.
