use crate::components::{Position, Speed, Target, Velocity};
use crate::systems::{movement, steering};
use enki::{Entity, World};
use glam::Vec3;
use std::time::Duration;

pub struct Simulation {
    world: World,
    hand: Entity,
    unsimulated: Duration, // time that has passed and that no step has used yet, always below `STEP`
}

impl Simulation {
    // The length of one step, one sixtieth of a second, (60hz)
    pub const STEP: Duration = Duration::from_nanos(16_666_667);
    // The most time that one call to `advance` simulates
    const MAX_ELAPSED: Duration = Duration::from_millis(250);
    const HOVER_HEIGHT: f32 = 1.0;
    const HAND_SPEED: f32 = 20.0;

    pub fn new() -> Self {
        let mut world = World::new();
        let hand = world.spawn_empty();
        let start = Vec3::new(0.0, Self::HOVER_HEIGHT, 0.0);
        let components = (
            Position(start),
            Velocity(Vec3::ZERO),
            Target(start),
            Speed(Self::HAND_SPEED),
        );
        world
            .add_components(hand, components)
            .expect("the hand should exist");
        Self {
            world,
            hand,
            unsimulated: Duration::ZERO,
        }
    }

    pub fn set_hand_target(&mut self, ground_point: Vec3) {
        // Lifts the point on the ground to the hover height
        let hover_point = ground_point + Vec3::Y * Self::HOVER_HEIGHT;
        let target = self
            .world
            .get_mut::<Target>(self.hand)
            .expect("the hand always has a target");
        target.0 = hover_point;
    }

    pub fn find_hand_position(&self) -> Vec3 {
        let position = self
            .world
            .get::<Position>(self.hand)
            .expect("the hand always has a position");
        position.0
    }

    pub fn advance(&mut self, elapsed: Duration) {
        self.unsimulated += elapsed.min(Self::MAX_ELAPSED);
        while self.unsimulated >= Self::STEP {
            self.step();
            self.unsimulated -= Self::STEP;
        }
    }

    fn step(&mut self) {
        let step_seconds = Self::STEP.as_secs_f32();
        steering(&mut self.world, step_seconds);
        movement(&mut self.world, step_seconds);
    }
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new()
    }
}
