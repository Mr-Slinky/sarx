---
status: accepted
date: 2026-11-01
decision-makers: Kheagen Haskins
---

# Miri for Unsafe Code

## Context and Problem Statement

`ComponentVec` in `sarx-ecs` stores components as raw bytes. It manages its own block of memory
through `unsafe` code. A mistake in that code can cause undefined behaviour, such as reading freed
memory or dropping a component twice.

Undefined behaviour does not always produce a visible symptom. A test that runs under plain
`cargo test` can therefore pass despite it.

Miri is a Rust tool that runs a test program in an interpreter. The interpreter checks each memory
access. It stops the test at the first fault that it detects. Miri runs only on the nightly
toolchain. Sarx otherwise uses stable releases only.

Do the tests for `unsafe` code also run under Miri?

## Decision Drivers

- The simulation and the physics engine both build on the ECS. As a result, a memory fault in the
  ECS reaches both.
- The compiler does not check that `unsafe` code uses memory correctly. The tests are therefore the
  main check on it.
- Sarx uses stable releases of its toolchain and its dependencies.

## Considered Options

- Plain `cargo test` plus Miri on the nightly toolchain
- Plain `cargo test` alone, on the stable toolchain

## Decision Outcome

Chosen option: "Plain `cargo test` plus Miri on the nightly toolchain", because a passing test can
hide a fault in `unsafe` code, and Miri detects that fault.

The nightly toolchain is an exception to the stable-only practice. The exception applies to the Miri
check alone. Miri runs only the `sarx-ecs` tests. `rust-toolchain.toml` selects stable as the
toolchain that builds and runs Sarx.

### Consequences

- Good, because a test that causes a use after free, a double free, a memory leak or a misaligned
  access fails under Miri.
- Good, because the stable toolchain alone builds the game itself.
- Bad, because the nightly toolchain changes daily. As a result, a Miri run can start to behave
  differently with no change in Sarx.
- Bad, because Miri runs code far more slowly than a plain `cargo test` run.
- Bad, because Miri checks only the code that a test reaches. A fault on a path that no test takes
  stays hidden.

### Confirmation

`cargo +nightly miri test -p sarx-ecs` passes, and `rust-toolchain.toml` selects the stable channel.

## Pros and Cons of the Options

### Plain `cargo test` Plus Miri on the Nightly Toolchain

- Good, because the tests check both the results and the memory accesses behind those results.
- Bad, because a developer has to install a second toolchain and keep it up to date.

### Plain `cargo test` Alone, on the Stable Toolchain

Tests would place a counter inside a test component. The counter would count each drop of that
component.

- Good, because the project would use one toolchain only.
- Good, because a drop counter detects a double drop or a missed drop on the stable toolchain.
- Bad, because a drop counter detects faults in dropping alone. A test that reads freed memory can
  still pass.
