use enki::Component;
use glam::Vec3;

/// The point that an entity occupies in the world, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position(pub(crate) Vec3);

/// The direction and speed that an entity moves at, in world units per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub(crate) Vec3);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target(pub(crate) Vec3);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speed(pub(crate) f32);

impl Component for Position {}
impl Component for Velocity {}
impl Component for Target {}
impl Component for Speed {}
