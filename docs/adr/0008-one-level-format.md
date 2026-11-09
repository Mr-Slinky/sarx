---
status: accepted
date: 2026-10-18
decision-makers: Kheagen Haskins
---

# One Level Format

## Context and Problem Statement

A player starts a level in one of two ways. They can configure a level, choosing settings such as
the number of factions and the kind of landscape. They can also pick a pre-made level. Either way,
the level then has to reach the simulation as a world that it can run.

A configured level comes from a generator. The generator is a piece of code that builds a level from
the player's settings and a seed. A seed is a number that fixes every random choice that the
generator makes.

A pre-made level comes from a person, who builds it by hand ahead of time.

How does a level reach the game, whether the player configured it or picked a pre-made one?

## Decision Drivers

- The player configures a level or picks a pre-made one. The game loads either kind.
- A person builds pre-made levels by hand. They can start from a generated level in place of an
  empty one.
- The same settings and seed always give the same level.

## Considered Options

- One level format, loaded through one path
- Separate loaders for generated and hand-built levels
- Every level as settings and a seed

## Decision Outcome

Chosen option: "One level format, loaded through one path", because the game then loads a pre-made
level and a configured level through the same code.

The game loads every level from data in a single format, whatever produced it.

A generator produces a level from the player's settings and a seed. The same settings and seed
always give the same level.

A person uses a separate tool to build a level by hand, in that same format. With the tool, the
person can also refine a generated level.

The format itself remains open. Three questions about it wait for later: whether it is text or
binary, how it is serialised, and which library reads and writes it. Any library for it goes through
the discussion that the Libraries section of the README sets out. The outcome of that discussion
gets an ADR of its own.

### Consequences

- Good, because the game has one loading path. The tests for that path cover generated and
  hand-built levels alike.
- Good, because a generated level is data in the same format as a hand-built one. A person can
  therefore open it in the tool and refine it.
- Good, because a configured level can be rebuilt from its settings and seed. The rebuild gives the
  same level each time.
- Bad, because the format has to express everything that the generator or the tool can produce. A
  feature that either one adds needs a place in the format first.
- Bad, because the generator writes level data, rather than building the state of the simulation
  directly. This adds a step between generating a level and playing it.
- Neutral, because the generator has to draw every random choice from the seed. A random choice
  taken from anywhere else, such as the clock, would give a different level for the same seed.

### Confirmation

Nothing in the code embodies this decision yet, since no level code exists. Once it does, a reader
checks three points:

- The game loads every level through one function, which accepts level data in the single format.
- The generator and the tool both produce that data. Neither one passes a level to the game by any
  other route.
- A test generates a level twice from the same settings and seed, then compares the two. The two
  levels match.

## Pros and Cons of the Options

### One Level Format, Loaded Through One Path

The generator and the tool both write level data in one format. The game reads that data with one
loader.

- Good, because one loader serves every level.
- Good, because the tool can open and refine anything that the generator produces.
- Bad, because the format grows with every feature that the generator or the tool gains.
- Bad, because generation passes through the format before the simulation receives the level.

### Separate Loaders for Generated and Hand-Built Levels

The generator builds the world straight into the state of the simulation when a level starts. A
second loader reads a hand-built level from a file.

- Good, because a generated level reaches the simulation without being written out as data first.
- Bad, because two paths each set up the same world. The two can drift apart as levels gain
  features.
- Bad, because a generated level exists only inside the running game. The tool then has nothing to
  open when a person wants to refine one.

### Every Level as Settings and a Seed

A pre-made level is stored as a fixed set of settings and a seed. The game runs the generator on
them whenever the level loads.

- Good, because a pre-made level is stored as a few values. Every level also passes through the generator.
- Bad, because a person shapes a level only through the settings of the generator. A hill or a
  village placed by hand lies outside what the settings can express.
- Bad, because any change to the generator changes every pre-made level built on it.
