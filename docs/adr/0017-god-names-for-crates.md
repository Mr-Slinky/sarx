---
status: accepted
date: 2026-11-08
decision-makers: Kheagen Haskins
---

# God Names for Crates

## Context and Problem Statement

Sarx is split into four crates: `sarx-ecs`, `sarx-sim`, `sarx-render` and `sarx-app`. Each name
states the job of its crate in a word or two. The names work, but they give the project no character.

Sarx is a god game. The developer wants crate names that suit one, and that make the project more
fun to work on.

The game Horizon Zero Dawn gave the idea. In its story, the Zero Dawn project names each of its
subsystems after a god, mostly from the Greek pantheon. Each god has a known domain, and that domain
matches the job of the subsystem. HEPHAESTUS, the smith of the gods, builds the machines.

How does Sarx name its crates?

## Decision Drivers

- Each name must suit a god game.
- Each name must point to the job of its crate, through a meaning that a reader can look up.
- The convention must extend to every crate that the developer adds later.
- A developer types each name in commands such as `cargo run -p`, so a shorter name is better.

## Considered Options

- Names that state the job, with the `sarx-` prefix
- God names from one pantheon
- God names from any pantheon, with the `sarx-` prefix
- God names from any pantheon, without a prefix

## Decision Outcome

Chosen option: "God names from any pantheon, without a prefix", because the developer can then pick,
for each crate, the god whose domain fits its job best.

The four crates take these names:

| Before        | After       | God                                    |
|---------------|-------------|----------------------------------------|
| `sarx-ecs`    | `enki`      | Enki, Mesopotamian god of wisdom       |
| `sarx-sim`    | `zurvan`    | Zurvan, Persian god of infinite time   |
| `sarx-render` | `amaterasu` | Amaterasu, Japanese goddess of the sun |
| `sarx-app`    | `atum`      | Atum, the first Egyptian god           |

The Crate Names section of the README explains why each god fits its crate. A crate that the
developer adds later takes a god in the same way. The developer chooses the god once the job of
that crate is clear.

### Consequences

- Good, because the names suit a god game.
- Good, because each god has a known domain, which points to the job of its crate.
- Good, because each pantheon offers many gods. A new crate can therefore take whichever god fits
  its job, from anywhere.
- Good, because the names are shorter to type than the old ones.
- Bad, because a name alone does not tell a new reader what its crate does. The reader must look the
  crate up in the README.
- Bad, because a library that Sarx takes on later could share its name with a crate. The developer
  would then rename that dependency in `Cargo.toml`, through its `package` key.

### Confirmation

A reader confirms this decision with two checks:

- Each folder under `crates/` takes the name of a god, without a prefix. The `name` in its
  `Cargo.toml` matches the folder.
- The Crate Names section of the README lists every crate, with the god behind it.

## Pros and Cons of the Options

### Names That State the Job, with the `sarx-` Prefix

The crates would keep the names that they had before this decision.

- Good, because each name states the job of its crate.
- Bad, because the names give the project no character.

### God Names from One Pantheon

Every crate would take a god from the same pantheon, such as the Greek or the Norse.

- Good, because the names would form one set.
- Bad, because one pantheon rarely has a god whose domain fits every crate.
- Bad, because the Greek and Norse gods already name a great deal of software.

### God Names from Any Pantheon, with the `sarx-` Prefix

Each crate would take a god from any pantheon, behind the `sarx-` prefix, as in `sarx-enki`.

- Good, because the prefix marks each crate as part of Sarx.
- Good, because the prefix makes a clash with the name of a library less likely.
- Bad, because every name grows by five characters. The prefix tells a developer nothing inside the
  workspace, where every crate belongs to Sarx.

### God Names from Any Pantheon, without a Prefix

The Decision Outcome sets out this option.

- Good, because each crate can take the god that fits it best.
- Good, because the names are short.
- Bad, because a name alone does not tell a new reader what its crate does.
