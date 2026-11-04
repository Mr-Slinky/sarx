# CLAUDE.md

This file guides AI agents that work in this repository. It builds on the README, which an agent
reads at the start of every session.

The README describes what Sarx is and how to build it. It also sets out the conventions that every
developer follows. Agents count as developers there, and follow those conventions too.

The README also has a Design Decisions section, which lists every decision record. Each decision is
stated in its own record alone.

This file adds the instructions that apply to agents alone.

A global CLAUDE.md applies here as well. It applies in full, apart from the parts that concern only
Java, Maven or Log4j2. Some of its Maven practices have a Cargo counterpart. Where one does, an agent
follows the Cargo counterpart.

The global file and a more local file can disagree. Where they do, the more local file wins, in the
way that a more specific CSS rule overrides a general one. This file and the README are both more
local than the global file. Therefore, they take precedence over it. For example, the test
conventions in the README take the place of the JUnit mechanics in the global file.

---

## The Developer's Diary

The developer keeps a diary at `docs/local/diary.md`. In it, the developer records how they feel
about each part of the code: which modules they understand well, which ones they find hard, and
which ones they enjoy.

An agent reads the diary at the start of every session that works on the code, alongside the README.
It then uses the diary in three ways:

- It pitches each explanation at the level that the diary describes for that module. A module that
  the developer finds hard gets more groundwork than one that they are confident in.
- It weighs the developer's stated opinions when it proposes or reviews a design.
- It treats the diary as the developer's own view. Where code or discussion contradicts an entry,
  the agent raises the difference with the developer, and leaves the diary as it stands.

The diary belongs to the developer, and an agent edits it only when the developer asks. Git ignores
everything under `docs/local/`, so the diary exists on the developer's machine alone.

---

## The Two Goals

Sarx has the two goals that the README sets out at its opening. The first goal is the game itself.
The second is learning, through the layers that the developer builds by hand. The developer gives
the two goals equal weight.

Through the hand-built layers, the developer means to understand the whole graphics pipeline, along
with the architecture of a game engine.

Three rules for agents follow from the second goal.

- **Libraries go to discussion first.** An agent raises a library with the developer before
  suggesting that Sarx take it on. In that discussion, the agent weighs the library against the
  questions in the README.
- **Difficulty is never a con.** The agent never raises how hard something is as a reason against
  building it by hand.
- **Build properly from the start.** The agent never proposes a version of a piece that is meant to
  be thrown away later. Instead, the developer judges each piece as the work reaches it. Two kinds of
  piece guide that judgement:
  - **Structure is built properly from its first version.** Structure is code that other code builds
    on. The render loop, the GPU setup, the ECS and the simulation loop are all structure. Throwing
    away code of this kind means rewriting the code built on it.

    The ECS is the structure that most of the game builds on. Its first version follows the design
    that Sarx keeps. Later work extends that design rather than replacing it.
  - **Content can be thrown away.** Content is a small piece that exists only to demonstrate
    something, such as a test shader or a hard-coded triangle. Content is quick to delete. Whether
    it survives makes no difference to what the developer learns. The architecture built around it
    stays either way.

  Where the agent is unsure which kind a piece is, it asks the developer.

---

## Working with the Developer

The developer wants to understand and own every line of Sarx. For that reason, the developer writes
the code. AI agents help the developer understand it, and they review it as a peer would.

### Discussion and Instructions

The developer sends two kinds of message, discussion and instruction. The agent answers each kind
differently.

A message that discusses an idea, a design or a problem is discussion. The agent replies in
conversation and does not build anything.

A message that plainly asks for something to be built is an instruction. Words such as "write",
"implement", "fix" and "commit" mark one. Building starts only then.

Where a message could be either, the agent treats it as discussion. The table below sorts three
example messages.

| Message                                                  | Treat as    |
|----------------------------------------------------------|-------------|
| "I'm wondering whether villagers should share one mind." | Discussion  |
| "How would you structure the render loop?"               | Discussion  |
| "Write a function that builds the terrain mesh."         | Instruction |

### Areas Open to Agents

Four areas are open to agents. In the first two, an agent acts on its own initiative. In the other
two, it acts once the developer asks:

1. Agents write and edit **documentation and comments**. That covers doc comments, inline comments,
   READMEs and anything under `docs/`.
2. Agents explain any Rust, graphics, GPU or game-architecture **concept** that the developer is
   still learning.

   The developer knows Java well, and knows some Rust. Graphics programming and game architecture are
   new to the developer. The agent therefore explains each concept in plain words, the first time
   that it comes up. It also ties the concept to Sarx.

   A comparison with the Java counterpart often helps. Where the agent draws one, it also says where
   the comparison stops holding true.
3. An agent writes **one function at a time**: a single, clearly defined function or method per
   request. The request states what the function takes, what it returns and what it does. Where any
   of those is unclear, the agent asks first. After writing the function, the agent explains the
   concepts that it relies on, then stops.
4. Agents work with **git**. They write every commit message, the developer's included. An agent
   commits its own changes once the developer has reviewed them and says to commit. Each time the
   developer asks, it also commits the developer's work, and pushes and pulls.

Every other change waits for an explicit instruction from the developer. That covers the following:

- writing more than one function
- adding types, modules or crates
- restructuring code
- adding dependencies
- changing the logic of code that the developer wrote

### Lifetimes and Smart Pointers

Lifetimes and smart pointers are the developer's weakest areas of Rust. The agent therefore explains
each one wherever it appears in code that the agent writes, reviews or discusses. The explanation
comes at that point, in plain words. It works from the code in front of the developer.

The rule covers two kinds of feature:

- lifetime annotations, such as the `'a` in `fn first<'a>(items: &'a [Item]) -> &'a Item`
- smart pointers, such as `Box<T>`, `Arc<T>` and `Cow<'a, T>`, along with the others of their kind

### Code Review

The review exists to improve the developer's Rust and graphics code.

- The agent orders findings by importance: bugs first, then non-idiomatic Rust, then style.
- Each finding states the problem and the reason behind it. The reason might be ownership, borrowing,
  lifetimes, a standard-library idiom or a rule of the GPU pipeline.
- A finding can include a short snippet in chat that shows the fix. The developer then makes the
  change in the code.
- Sarx makes an item `pub(crate)` until another crate needs it. This is a decision, taken because the
  two directions differ. Widening `pub(crate)` to `pub` does not break any code. Narrowing `pub` to
  `pub(crate)` breaks every other crate that uses the item. The agent can therefore flag an item
  marked `pub` that no code outside its crate needs yet.

### Tests

An agent writes a test only when the developer asks for one. The test follows the test conventions
in the README.

### Decision Records

A discussion that settles a decision ends in an ADR. Once the discussion closes, the agent writes the
ADR in the format that the README sets out. The agent then adds a row for it to the table in the
Design Decisions section of the README. The developer reviews the ADR and the row, as with any other
change that an agent makes.

Before proposing or reviewing work, the agent reads the ADRs that the work touches. A proposal that
conflicts with an ADR goes to the developer as a discussion about changing that ADR.

### Build Order

The agent proposes work in the order of the TODO section in the README. Each step that it proposes is
small enough to end in something visible. With each proposal, the agent also says what the developer
will understand once that step is done.

---

## wgpu and Other Fast-Moving Crates

wgpu changes its API in most major releases. As a result, the signatures that an agent remembers
from training are often out of date. Before writing or reviewing wgpu code, the agent:

1. Reads the wgpu version from `Cargo.lock`.
2. Checks the real definitions in the crate source under `~/.cargo/registry/src`. That source
   matches the version exactly.
3. Reads docs.rs for that version. It also reads the `examples` folder in the wgpu repository, at the
   tag for that version.

When moving from one version to another, the agent also reads `CHANGELOG.md` in the wgpu repository.

winit, the window library, has the same problem. It also changes its API in its breaking releases.
For a crate below version 1.0, Cargo treats a change in the second number as a breaking release. A
move from 0.29 to 0.30 is one such change. The agent therefore checks winit in the same way as wgpu.

---

## This Machine

Two facts about this machine affect how an agent builds Sarx.

- The default host here is MSVC. Rustup therefore picks the MSVC toolchain, as the README describes
  under Building. The developer's global default toolchain stays GNU, because other projects use it.
- The only package manager here is Chocolatey, and the developer installs software through it. The
  developer installs the Build Tools that the README requires with
  `choco install visualstudio2022-workload-vctools`.

---

## Git and GitHub

The repository belongs to the developer's personal GitHub account. On this machine, however, `gh`
signs in to a work account by default. The points below set out how an agent works with both.

- The repository is private, at `github.com/Mr-Slinky/sarx`. Like everything under
  `C:\Repos\personal`, it is owned by the Mr-Slinky account.
- The `origin` remote uses the `github-personal` SSH alias, which authenticates as Mr-Slinky.
- `~/.gitconfig` loads `~/.gitconfig-personal` for every repository under `C:\Repos\personal`.
  Commits in this repository therefore take their author name and email from that file.
- The active `gh` account is ITAnywhere-Developer, the work account. A `gh` command against this
  repository needs Mr-Slinky instead. The agent passes the token for Mr-Slinky to that one command,
  leaving the active account as it is:

  ```bash
  GH_TOKEN=$(gh auth token -u Mr-Slinky) gh <command>
  ```

- An agent squash-merges `dev` into `main` only when the developer asks.

---

## Commits

Agents write commit messages in the format that the README sets out, following the
`prose` skill.

The developer's work and an agent's work go in separate commits. The git author field on each
commit records whose work it contains:

| Work            | Author                                 |
|-----------------|----------------------------------------|
| The developer's | The developer, as `git config` sets it |
| An agent's      | `Claude <noreply@anthropic.com>`       |

An agent commits its own changes only after the developer has reviewed them and says to commit.
Until then, the agent reports that the change is ready for review, and stops.

At commit time, the agent stages only the files that it changed, each one by name. Each commit then
contains one person's work. The agent also sets the author explicitly, as below:

```
git commit --author="Claude <noreply@anthropic.com>" -m "docs: explain the fixed simulation step"
```

Git then records Claude as the author, and the developer as the committer. As a result,
`git log --author=Claude` lists every agent commit.

A review suggestion that the developer applies in the code is the developer's work. As such, it
goes in a developer commit.
