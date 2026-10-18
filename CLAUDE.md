# CLAUDE.md

This file guides AI agents working in the Sarx repository. The global CLAUDE.md still applies, apart
from its Java, Maven and Log4j2 sections. Cargo takes the place of Maven here.

The README states what Sarx is, the planned features, the design decisions, the crate layout and the
TODO list. Read it at the start of every session, because this file relies on it rather than
repeating it.

---

## The Two Goals

Sarx is a game and a course of study at once, and the second goal weighs as much as the first. The
developer is building every layer below the gameplay by hand, to understand the whole graphics
pipeline and the architecture of a game engine. The developer expects the project to run for several
years.

- **Bevy and other game engines are out.** An engine does the work that the developer wants to
  learn.
- **Each library is a decision of its own.** Where a library exists for a job, the agent and the
  developer discuss it and weigh its pros and cons before Sarx takes it on. Three questions decide
  the outcome:
  - **Functions or architecture.** A library of separate functions leaves Sarx's design alone. glam
    is the example, since how it computes a dot product has no bearing on the game. A library that
    brings its own architecture commits Sarx to someone else's opinions, as an ECS library would.
  - **What it hides.** A library that hides a part the developer wants to learn and control stays
    out. An ECS library hides the ECS, for example. wgpu hides the differences between Vulkan, Metal
    and DirectX, and leaves the pipeline, shaders, buffers and draw calls in the developer's hands.
  - **Labour.** The volume of work that a library saves counts in its favour. Hand-writing the maths
    that glam provides would take long enough to stall the project.
- **Difficulty is never a con.** The agent never raises how hard something is as a reason against
  building it by hand.
- **Build to extend, wherever possible.** The agent proposes a small, runnable MVP that teaches a
  concept, and the developer extends it as the concept lands. Throwaway code is the exception, kept
  for a piece where building something to extend proves too difficult. The agent judges which kind
  each piece is, using one test:
  - **Structure is extended.** Code that other code builds on, such as the render loop, the GPU
    setup, the ECS and the simulation loop, starts as an MVP and grows from there.
  - **Content can be thrown away.** A small piece that exists only to demonstrate something, such as
    a test shader or a hard-coded triangle, goes once it has served its purpose.

  The agent says which kind a piece is before the developer builds it. Where the call is unclear,
  the agent discusses it with the developer.

---

## Workspace

Sarx is a Cargo workspace. Each crate lives in its own folder under `crates/`, and the root
`Cargo.toml` picks up every folder there as a member. The commands below run from the repository
root.

| Task              | Command                             |
|-------------------|-------------------------------------|
| Build every crate | `cargo build`                       |
| Run the game      | `cargo run -p sarx-app`             |
| Test every crate  | `cargo test`                        |
| Build one crate   | `cargo build -p <crate>`            |
| Lint every crate  | `cargo clippy --workspace`          |
| Add a dependency  | `cargo add -p <crate> <dependency>` |

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

Agents have free rein in four areas:

1. **Documentation and comments.** Agents write and edit doc comments, inline comments, READMEs and
   anything under `docs/`.
2. **Concepts.** Agents explain any Rust, graphics, GPU or game-architecture concept the developer
   is still getting to grips with. The developer knows Java well and some Rust. Graphics programming
   and game architecture are new, so the agent explains each concept in plain words on first use and
   ties it to Sarx. A comparison with the Java counterpart often helps. Where the agent draws one, it
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
changing the logic of code the developer wrote.

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
  the default until another crate needs the item. This is a decision based on the cost of changing
  visibility later. Widening `pub(crate)` to `pub` breaks no code, while narrowing `pub` to
  `pub(crate)` breaks every other crate that uses the item.

### Tests

An agent writes a test only when the developer asks for one. The ideas behind the global testing
standards carry over to Rust, with Rust's own mechanics in place of the Java ones.

- **Location.** Unit tests go in a `#[cfg(test)] mod tests` block at the end of the file that they
  test, as is standard in Rust.
- **Data.** The tests for a function give it standard data, extreme data at the edges of what it
  accepts, and abnormal data that it must reject.
- **Cases in a table.** Where the same assertion repeats across inputs, one test covers every input,
  in place of a copy of the test for each. Rust's built-in test framework has no parameterised
  tests. The `rstest` crate adds them through `#[rstest]` and `#[case]`, and Sarx takes it on only
  after a decision, like any other library. Until then, a test loops over an array of cases, and
  each assertion message states the case that failed.
- **Arrange, act, assert.** Each test sets up, acts, then asserts, with every assertion at the end.
- **Naming.** A test name follows the global pattern in snake case, as
  `{function}_with_{data}_{outcome}`. An example is `height_at_with_point_outside_map_returns_none`.

### Decision Records

Each discussion that settles a decision ends in an Architecture Decision Record (ADR), a short
document that records what the developer decided and why. The agent writes the ADR once the
discussion closes, and the developer reviews it as any other agent change.

- ADRs live in `docs/adr/`, one file per decision, numbered in order, such as
  `0001-window-library.md`.
- Each ADR states its status, then covers four parts: the context, the options weighed with their
  pros and cons, the decision, and its consequences for Sarx.
- An accepted ADR stays as written. Where a later discussion changes a decision, a new ADR records
  the change. The old one is then marked as superseded, with a link to the new one.

### Build Order

The work follows the order in the README's TODO section. First comes a thin path through every
layer, from the window down to the GPU and back, ending in lit terrain on a phone and a desktop.
Each layer then widens on its own. The agent proposes work in that order, with each step small
enough to end in something visible, and says what the developer will understand once it is done.

---

## wgpu and Other Fast-Moving Crates

wgpu changes its API in most major releases, so remembered signatures are often out of date. Before
writing or reviewing wgpu code, the agent:

1. Reads the wgpu version from `Cargo.lock`.
2. Checks the real definitions in the crate source under `~/.cargo/registry/src`, which matches that
   version exactly.
3. Uses docs.rs for that version, the `examples` folder in the wgpu repository, and its
   `CHANGELOG.md` when moving between versions.

winit follows the same pattern and gets the same treatment.

---

## Toolchain

- `rust-toolchain.toml` selects stable Rust with rustfmt, clippy and the `aarch64-linux-android`
  target. On this Windows machine it resolves to the MSVC toolchain, because the default host is
  MSVC. The developer's global default toolchain is GNU, which other projects use, so it stays as it
  is.
- The MSVC toolchain needs the Visual Studio Build Tools with the C++ workload. The developer
  installs software through Chocolatey (`choco install visualstudio2022-workload-vctools`), and
  winget is absent from this machine.

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

- Development happens on `dev`. The developer squash-merges finished stages into `main`, and an
  agent does so only when the developer asks.

---

## Commits

Commits follow the Conventional Commits format from the `technical-writing` skill.

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
