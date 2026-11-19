//! Runs the simulation, which advances the game world in fixed steps of game time.
//!
//! The simulation keeps its entities in an `enki` [`World`](enki::World). So far, the world
//! contains one entity, the god's hand, which has a position, a velocity, a target, a top speed,
//! and the rotations of its bones.
//!
//! The example on [`Simulation`] moves the hand to a point, then holds and releases a pose.
//!
//! # Steps
//!
//! Each step covers the same length of game time, [`Simulation::STEP`], on every device, as
//! ADR 0004 sets out. A caller passes [`Simulation::advance`] the real time that has passed since
//! its last call. The simulation then runs one step for each whole step that the unused time
//! contains, and keeps the remainder for the next call.
//!
//! A system is a function that updates every entity with a given set of components. On each step,
//! the simulation runs four systems, in this order:
//!
//! - `steering` points each velocity at its target, no faster than the entity's top speed.
//! - `movement` moves each position by its velocity.
//! - `posing` turns each bone part of the way towards its goal rotation.
//! - `idling` gives the hand its next idle pose, once every bone has reached the current one.
//!
//! # Poses
//!
//! A [`Pose`] gives each bone of the hand a rotation, relative to its parent bone. The bones turn
//! towards one pose at a time, which is their goal, as ADR 0020 sets out. While no pose is held,
//! the hand cycles through its idle poses. [`Simulation::hold_pose`] makes an action pose the goal
//! and pauses the cycle. [`Simulation::release_pose`] then resumes the cycle from an idle pose that
//! it picks at random.

mod components;
mod pose;
mod random;
mod simulation;
mod systems;

pub use pose::{HandPoses, Pose};
pub use simulation::Simulation;
