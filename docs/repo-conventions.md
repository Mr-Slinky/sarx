# Repository Conventions

Everyone who works on the project follows the conventions below.

## Libraries

Before adding a library, the developer discusses it and weighs its pros and cons, and an ADR then
records the outcome. The next section sets out how an ADR is written.

Sarx does not take on a game engine, such as Bevy, since an engine does the very work that the
project exists to teach.

For every other library, three questions decide whether it comes in:

- **Does it bring its own architecture?** Some libraries are a set of separate functions, which
  leave the design of the game as it is. glam, a library of vector and matrix maths, is one example,
  since the way that it computes a dot product has no bearing on the game.

  A library that brings its own architecture, as an ECS library would, commits Sarx to the design
  that its own authors chose.
- **What does it hide?** Every library hides some part of the work. Where that part is one that the
  developer wants to learn and control, the library stays out. An ECS library, for example, hides
  the ECS.

  wgpu passes this test. It hides the differences between Vulkan, Metal and DirectX. The pipeline,
  shaders, buffers and draw calls stay in code that the developer writes.
- **How much work does it do?** Work that a library does in place of hand-written code counts in its
  favour. glam is again the example. Writing its maths by hand would delay every layer that depends
  on that maths.

## Architectural Decisions

Every ADR uses the same format, [MADR](https://adr.github.io/madr/) version 4. The points below set
out how Sarx applies that format.

- Each ADR is a file of its own in `docs/adr/`, numbered as in `0001-window-library.md`.
- Each ADR opens with a YAML block. The block records the status of the ADR, its date and the people
  who made the decision. The status is `accepted` once code acts on the decision, and `proposed`
  until then.
- The accepted ADRs take the lowest numbers, and the proposed ADRs follow them. The numbers do not
  follow the dates, since each ADR records its own date. A new ADR that code acts on straight away
  takes the number after the last accepted ADR, and every proposed ADR moves up one number to make
  room. A proposed ADR that becomes accepted moves down in the same way, to follow the last accepted
  ADR. Each move updates every reference to the numbers that change.
- The sections then follow in the order that MADR sets:
  1. the context and problem statement
  2. the decision drivers
  3. the options considered
  4. the decision outcome, with its consequences and its confirmation
  5. the pros and cons of each option
- For each new ADR, the developer adds a row to the table under
  [Decision Records](../README.md#decision-records) in the README.
- An ADR always records the current decision. Where a decision changes, the developer rewrites its
  ADR in place and updates the date. Git keeps the earlier versions.

## Naming

Every function and method name starts with a verb that says what it does, such as
`find_or_create_archetype`, `read_positions` or `fetch_row`. A predicate starts with `is`, `has` or
`can`, as in `has_all`.

Rust's own naming conventions take precedence over this rule wherever the two differ. The
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) set those conventions
out. The case that arises most often is a getter. A getter takes the name of what it returns, with
no `get_` prefix. `Archetype::len`, `Archetype::mask` and `Entity::index` follow that convention, as
`Vec::len` does in the standard library.

## Tests

Unit tests follow the standard Rust layout. Each file keeps its tests in a `#[cfg(test)] mod tests`
block at its end.

Every test also follows five conventions:

- The tests for a function give it three kinds of **data**. Standard data is ordinary input.
  Extreme data lies at the edges of what the function accepts. Abnormal data is input that the
  function must reject.
- Where the same assertion repeats across inputs, one test covers every input from a **table of
  cases**. That one test replaces a copy of the test for each input.

  The `rstest` crate provides tests of this kind, through `#[rstest]` and `#[case]`. Like any other
  library, though, it awaits a decision. Until then, a test loops over an array of cases. Its
  assertion message states which case failed.
- Each test follows **arrange, act, assert**. It sets up its data, then acts, then asserts. Every
  assertion comes at the end.
- A test **name** follows the pattern `{function}_with_{data}_{outcome}`, in snake case.
  `height_at_with_point_outside_map_returns_none` is one such name.
- A test that expects a **failure** checks that the failure happens, and leaves the wording of its
  message unchecked. A refactor or a fix can reword a message while the code stays correct. A test
  that checked the wording would then fail, and force a change to the test as well as the code. A
  test that expects a panic therefore takes a bare `#[should_panic]`, with no `expected` text. A
  test that expects an `Err` checks which error it got, and leaves the text that `Display` writes
  unchecked.

## Commits

Every commit message follows the Conventional Commits format, `<type>[scope]: <description>`. The
type is one of `feat`, `fix`, `docs`, `style`, `refactor`, `test` and `chore`.

## Branches

The developer develops on the `dev` branch, which keeps the full history of the work. That history
records the author of every commit, so anyone can audit which work came from an agent. The `main`
branch is the stable changelog. Each commit on `main` is one update, which brings in everything
that is new on `dev` since the update before.

### Bringing main up to date

GitHub accepts a change to `main` only through a pull request. The pull request also has to pass two
checks on `enki`, `cargo test` and `cargo miri test`. The workflow in `.github/workflows/enki.yml`
runs both checks on every pull request into `main`, and on every push to it.

A developer brings `main` up to date in four steps, starting from a clean working tree on `dev`.

1. Push `dev`, then open a pull request from `dev` into `main`:

   ```bash
   git push origin dev
   gh pr create --base main --head dev
   ```

2. Once both checks pass, merge the pull request with **Squash and merge**, and give the squash
   commit a message in the Conventional Commits format.

   A squash merge combines every commit in the pull request into one commit. GitHub signs that
   commit with its own key, and the commit therefore shows a Verified badge. The squash commit names
   the developer as its author, even where it contains an agent's work, since the developer opened
   the pull request.
3. Merge `main` back into `dev`, keeping the files on `dev` as they are:

   ```bash
   git fetch origin
   git merge -s ours origin/main
   git push origin dev
   ```

   The squash commit holds the same files as `dev`, so the merge changes no file. It adds the squash
   commit to the history of `dev`, and the next pull request from `dev` therefore shows only the
   commits made since.
4. Move the local `main` to match GitHub:

   ```bash
   git branch -f main origin/main
   ```
