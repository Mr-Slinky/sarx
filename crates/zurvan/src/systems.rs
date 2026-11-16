use crate::components::{Position, Speed, Target, Velocity};
use enki::World;

pub(crate) fn movement(world: &mut World, step: f32) {
    for (position, velocity) in world.query_mut::<(&mut Position, &Velocity)>() {
        position.0 += velocity.0 * step;
    }
}

pub(crate) fn steering(world: &mut World, step: f32) {
    // Guards callers inside this crate, which can pass any step. A step of 0 would divide the
    // offset by 0 and give every entity an infinite or NaN velocity
    assert!(step > 0.0, "the step must be longer than 0 seconds");
    for (velocity, position, target, speed) in
        world.query_mut::<(&mut Velocity, &Position, &Target, &Speed)>()
    {
        let offset = target.0 - position.0;
        velocity.0 = (offset / step).clamp_length_max(speed.0);
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use enki::Entity;
    use glam::Vec3;

    /// Sets the largest difference at which two floats count as equal
    const TOLERANCE: f32 = 1e-5;

    /// Returns a new entity in `world` with every component that `steering` needs, standing still
    fn spawn_mover(world: &mut World, position: Vec3, target: Vec3, speed: f32) -> Entity {
        let entity = world.spawn_empty();
        let components = (
            Position(position),
            Velocity(Vec3::ZERO),
            Target(target),
            Speed(speed),
        );
        world
            .add_components(entity, components)
            .expect("the entity was just spawned");
        entity
    }

    /// Returns the velocity of `entity`
    fn read_velocity(world: &World, entity: Entity) -> Vec3 {
        world
            .get::<Velocity>(entity)
            .expect("the entity has a velocity")
            .0
    }

    /// Returns the position of `entity`
    fn read_position(world: &World, entity: Entity) -> Vec3 {
        world
            .get::<Position>(entity)
            .expect("the entity has a position")
            .0
    }

    /// Checks that each position moves by its velocity times the step
    #[test]
    fn movement_with_velocity_moves_position_by_velocity_times_step() {
        let cases = [
            (
                "standard step",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(2.0, 0.0, -4.0),
                0.5,
            ),
            ("zero velocity", Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, 0.5),
            (
                "zero step",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(2.0, 0.0, -4.0),
                0.0,
            ),
            (
                "large velocity",
                Vec3::ZERO,
                Vec3::new(1e6, -1e6, 0.0),
                1.0 / 60.0,
            ),
        ];

        for (name, start, velocity, step) in cases {
            let mut world = World::new();
            let entity = world.spawn_empty();
            world
                .add_components(entity, (Position(start), Velocity(velocity)))
                .expect("the entity was just spawned");

            movement(&mut world, step);

            let expected = start + velocity * step;
            let actual = read_position(&world, entity);
            assert!(
                actual.abs_diff_eq(expected, TOLERANCE),
                "position for {name}: {actual}"
            );
        }
    }

    /// Checks that an entity with a position but no velocity stays where it is
    #[test]
    fn movement_with_entity_missing_velocity_leaves_position() {
        let start = Vec3::new(1.0, 2.0, 3.0);
        let mut world = World::new();
        let entity = world.spawn_empty();
        world
            .add_component(entity, Position(start))
            .expect("the entity was just spawned");

        movement(&mut world, 0.5);

        assert_eq!(read_position(&world, entity), start);
    }

    /// Checks that a target close enough to reach in one step gives the velocity that lands on it
    #[test]
    fn steering_with_target_within_reach_aims_to_land_on_target() {
        let mut world = World::new();
        let entity = spawn_mover(&mut world, Vec3::ZERO, Vec3::new(0.1, 0.0, -0.2), 20.0);

        steering(&mut world, 0.5);

        let expected = Vec3::new(0.2, 0.0, -0.4);
        let actual = read_velocity(&world, entity);
        assert!(actual.abs_diff_eq(expected, TOLERANCE), "velocity {actual}");
    }

    /// Checks that a target too far to reach in one step gives a velocity of the entity's speed,
    /// pointing at the target
    #[test]
    fn steering_with_target_out_of_reach_clamps_velocity_to_speed() {
        let cases = [
            (
                "along x",
                Vec3::new(100.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
            ),
            (
                "diagonal",
                Vec3::new(30.0, 40.0, 0.0),
                Vec3::new(6.0, 8.0, 0.0),
            ),
            (
                "negative",
                Vec3::new(0.0, -30.0, -40.0),
                Vec3::new(0.0, -6.0, -8.0),
            ),
        ];

        for (name, target, expected) in cases {
            let mut world = World::new();
            let entity = spawn_mover(&mut world, Vec3::ZERO, target, 10.0);

            steering(&mut world, 1.0);

            let actual = read_velocity(&world, entity);
            assert!(
                actual.abs_diff_eq(expected, TOLERANCE),
                "velocity for {name}: {actual}"
            );
        }
    }

    /// Checks that an entity already at its target, or one whose speed is 0, gets no velocity
    #[test]
    fn steering_with_nothing_to_cover_sets_zero_velocity() {
        let at_target = Vec3::new(2.0, 3.0, 4.0);
        let cases = [
            ("target at position", at_target, at_target, 20.0),
            ("speed of 0", Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 0.0),
        ];

        for (name, position, target, speed) in cases {
            let mut world = World::new();
            let entity = spawn_mover(&mut world, position, target, speed);
            world
                .get_mut::<Velocity>(entity)
                .expect("the entity has a velocity")
                .0 = Vec3::ONE;

            steering(&mut world, 0.5);

            let actual = read_velocity(&world, entity);
            assert!(
                actual.abs_diff_eq(Vec3::ZERO, TOLERANCE),
                "velocity for {name}: {actual}"
            );
        }
    }

    /// Checks that an entity without a speed keeps its velocity, while a complete entity beside it
    /// still steers
    #[test]
    fn steering_with_entity_missing_speed_skips_entity() {
        let mut world = World::new();
        let complete = spawn_mover(&mut world, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 20.0);
        let incomplete = world.spawn_empty();
        let components = (
            Position(Vec3::ZERO),
            Velocity(Vec3::ONE),
            Target(Vec3::new(1.0, 0.0, 0.0)),
        );
        world
            .add_components(incomplete, components)
            .expect("the entity was just spawned");

        steering(&mut world, 0.5);

        let complete_velocity = read_velocity(&world, complete);
        let incomplete_velocity = read_velocity(&world, incomplete);
        assert!(
            complete_velocity.abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), TOLERANCE),
            "velocity of the complete entity: {complete_velocity}"
        );
        assert_eq!(
            incomplete_velocity,
            Vec3::ONE,
            "velocity of the entity without a speed"
        );
    }

    /// Checks that a step of 0 is rejected
    #[test]
    #[should_panic]
    fn steering_with_zero_step_panics() {
        let mut world = World::new();
        spawn_mover(&mut world, Vec3::ZERO, Vec3::ONE, 20.0);

        steering(&mut world, 0.0);
    }

    /// Checks that a negative step is rejected
    #[test]
    #[should_panic]
    fn steering_with_negative_step_panics() {
        let mut world = World::new();
        spawn_mover(&mut world, Vec3::ZERO, Vec3::ONE, 20.0);

        steering(&mut world, -0.5);
    }

    /// Checks that a NaN step is rejected
    #[test]
    #[should_panic]
    fn steering_with_nan_step_panics() {
        let mut world = World::new();
        spawn_mover(&mut world, Vec3::ZERO, Vec3::ONE, 20.0);

        steering(&mut world, f32::NAN);
    }
}
