//! Defines the components that the simulation stores on its entities.

use enki::Component;
use glam::{Quat, Vec3};

/// A `Position` is the point that an entity occupies in the world, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position(pub(crate) Vec3);

/// A `Velocity` is the direction in which an entity moves and how fast it moves, in world units
/// per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub(crate) Vec3);

/// A `Target` is the point that an entity is trying to reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target(pub(crate) Vec3);

/// The fastest that an entity moves, in world units per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speed(pub(crate) f32);

/// The rotation of each bone right now, relative to its parent
#[derive(Clone, Debug, PartialEq)]
pub struct Bones(pub(crate) Vec<Quat>);

/// The rotation that each bone is turning towards
#[derive(Clone, Debug, PartialEq)]
pub struct PoseGoal(pub(crate) Vec<Quat>);

/// The idle pose that the hand is turning towards, or `None` while a key holds a pose
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Idle(pub(crate) Option<usize>);

impl Component for Position {}
impl Component for Velocity {}
impl Component for Target {}
impl Component for Speed {}
impl Component for Bones {}
impl Component for PoseGoal {}
impl Component for Idle {}