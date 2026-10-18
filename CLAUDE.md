# CLAUDE.md

This file guides AI agents working in the Sarx repository. The README states what Sarx is, its design
decisions, its workspace, how to build it and the conventions that every developer follows, agents
included. An agent reads it at the start of every session. This file adds the instructions that apply
to agents alone.

The global CLAUDE.md still applies, apart from its Java, Maven and Log4j2 sections. The README's test
conventions take the place of the Java mechanics in its testing standards.

---

## The Two Goals

Sarx is a game and a course of study at once, and the second goal weighs as much as the first. The
developer is building every layer below the gameplay by hand, to understand the whole graphics
pipeline and the architecture of a game engine.

- **Libraries go to discussion first.** An agent raises a library with the developer, and weighs it
  against the questions in the README, before suggesting that Sarx take it on.
- **Difficulty is never a con.** The agent never raises how hard something is as a reason against
  building it by hand.
- **Build to extend, wherever possible.** The agent proposes a small, runnable first version that
  teaches a concept, and the developer extends it as the concept becomes clear. Throwaway code is the
  exception, kept for a piece where building something to extend proves too difficult. The agent
  judges which kind each piece is, using one test:
  - **Structure is extended.** Code that other code builds on, such as the render loop, the GPU
    setup, the ECS and the simulation loop, starts as a first version and grows from there.
  - **Content can be thrown away.** A small piece that exists only to demonstrate something, such as
    a test shader or a hard-coded triangle, is deleted once it has served its purpose.

  The agent says which kind a piece is before the developer builds it. Where the call is unclear,
  the agent discusses it with the developer.

---

## Working with the Developer

The developer wants to understand and own every line of Sarx. The developer writes the code, and AI
agents help with that understanding and act as peer reviewers.

### Discussion and Instructions

A message that discusses an idea, a design or a problem gets a reply in conversation, and the agent
builds nothing. Building starts only when a message asks for it in plain words, such as "write",
"implement", "fix" or "commit". Where a message could be either, the agent treats it as discussion.

| Message                                                  | Treat as    |
|----------------------------------------------------------|-------------|
| "I'm wondering whether villagers should share one mind." | Discussion  |
| "How would you structure the render loop?"               | Discussion  |
| "Write a function that builds the terrain mesh."         | Instruction |

### Areas Open to Agents

Agents act without asking in four areas:

1. **Documentation and comments.** Agents write and edit doc comments, inline comments, READMEs and
   anything under `docs/`.
2. **Concepts.** Agents explain any Rust, graphics, GPU or game-architecture concept that the
   developer is still learning. The developer knows Java well and some Rust. Graphics programming and
   game architecture are new to the developer. The agent therefore explains each concept in plain
   words on first use and ties it to Sarx. A comparison with the Java counterpart often helps. Where the agent draws one, it
   also says where the comparison stops holding true.
3. **One function at a time.** An agent writes a single, clearly defined function or method per
   request. The request states what the function takes, what it returns and what it does. Where any
   of those is unclear, the agent asks first. After writing the function, the agent explains the
   concepts it relies on, then stops.
4. **Git.** Agents write every commit message, the developer's included. An agent commits its own
   changes once the developer has reviewed them, as the Commits section sets out. It commits the
   developer's work, pushes and pulls each time the developer asks.

Every other change waits for an explicit instruction from the developer. That includes writing more
than one function, adding types, modules or crates, restructuring code, adding dependencies and
changing the logic of code that the developer wrote.

### Lifetimes and Smart Pointers

Lifetimes and smart pointers are the developer's weakest areas of Rust. Whenever code that the agent
writes, reviews or discusses uses one of them, the agent explains it at that point, in plain words
and in terms of the code in front of the developer. This covers:

- lifetime annotations, such as the `'a` in `fn first<'a>(items: &'a [Item]) -> &'a Item`
- smart pointers, such as `Box<T>`, `Arc<T>` and `Cow<'a, T>`, along with the others of their kind

### Code Review

The review exists to improve the developer's Rust and graphics code.

- The agent orders findings by importance: bugs first, then non-idiomatic Rust, then style.
- Each finding states the problem and the reason behind it, such as ownership, borrowing,
  lifetimes, a standard-library idiom or a rule of the GPU pipeline.
- A finding can include a short snippet in chat that shows the fix. The developer then makes the
  change in the code.
- The agent can flag an item marked `pub` that no code outside its crate needs yet. `pub(crate)` is
  the default until another crate needs the item. This is a decision, taken because the two
  directions differ in what they break. Widening `pub(crate)` to `pub` breaks no code, while narrowing `pub` to
  `pub(crate)` breaks every other crate that uses the item.

### Tests

An agent writes a test only when the developer asks for one, following the test conventions in the
README.

### Decision Records

Each discussion that settles a decision ends in an ADR, in the format that the README sets out. The
agent writes the ADR once the discussion closes, and the developer reviews it as with any other agent
change.

### Build Order

The agent proposes work in the order of the README's TODO section, with each step small enough to
end in something visible. With each proposal, the agent says what the developer will understand once
the step is done.

---

## wgpu and Other Fast-Moving Crates

wgpu changes its API in most major releases. As a result, the signatures that an agent remembers
from training are often out of date. Before writing or reviewing wgpu code, the agent:

1. Reads the wgpu version from `Cargo.lock`.
2. Checks the real definitions in the crate source under `~/.cargo/registry/src`, which matches that
   version exactly.
3. Uses docs.rs for that version, the `examples` folder in the wgpu repository, and its
   `CHANGELOG.md` when moving between versions.

winit also changes its API between major releases, meaning the agent checks it in the same way.

---

## This Machine

- On this Windows machine, `rust-toolchain.toml` resolves to the MSVC toolchain, because the default
  host is MSVC. The developer's global default toolchain stays GNU, since other projects use it.
- The developer installs software through Chocolatey, the only package manager on this machine. The
  Build Tools that the README requires install with
  `choco install visualstudio2022-workload-vctools`.

---

## Git and GitHub

- The repository is private, at `github.com/Mr-Slinky/sarx`, owned by the Mr-Slinky account like
  everything under `C:\Repos\personal`.
- The `origin` remote uses the `github-personal` SSH alias, which authenticates as Mr-Slinky. Commits
  pick up the personal identity automatically.
- The active `gh` account on this machine is ITAnywhere-Developer, a work account. For a `gh` command
  against this repository, the agent passes Mr-Slinky's token for that command alone, leaving the
  active account as it is:

  ```bash
  GH_TOKEN=$(gh auth token -u Mr-Slinky) gh <command>
  ```

- An agent squash-merges `dev` into `main` only when the developer asks.

---

## Commits

Agents write commit messages in the format that the README sets out, following the
`technical-writing` skill.

The developer's work and an agent's work go in separate commits. The git author field on each
commit records whose work it contains:

| Work            | Author                                 |
|-----------------|----------------------------------------|
| The developer's | The developer, as `git config` sets it |
| An agent's      | `Claude <noreply@anthropic.com>`       |

An agent commits its own changes only after the developer has reviewed them and says to commit.
Until then, the agent reports that the change is ready for review and stops. At commit time, the
agent stages only the files it changed, each by name, meaning each commit contains one person's
work. It also sets the author explicitly:

```
git commit --author="Claude <noreply@anthropic.com>" -m "docs: explain the fixed simulation step"
```

Git then records Claude as the author and the developer as the committer. `git log --author=Claude`
lists every agent commit.

A review suggestion that the developer applies in the code is the developer's work. As such, it
goes in a developer commit.
