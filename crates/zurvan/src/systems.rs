//! Defines the systems that the simulation runs on each step, in the order that the crate
//! documentation sets out.

use crate::components::{Bones, Idle, PoseGoal, Position, Speed, Target, Velocity};
use enki::World;
use crate::Pose;

// The angle, in radians, below which a bone snaps onto its goal
const SNAP_ANGLE: f32 = 0.5_f32.to_radians();

/// Moves each entity that has a `Position` and a `Velocity` by its velocity times `step_seconds`.
pub(crate) fn movement(world: &mut World, step_seconds: f32) {
    for (position, velocity) in world.query_mut::<(&mut Position, &Velocity)>() {
        position.0 += velocity.0 * step_seconds;
    }
}

/// Sets the velocity of each entity that has a `Position`, a `Target` and a `Speed`.
///
/// The velocity is the one that would carry the entity onto its target in one step of
/// `step_seconds`, shortened to the entity's speed if it is faster. An entity within one step of
/// its target therefore lands on it, and a further one moves towards it at full speed.
///
/// # Panics
///
/// Panics if `step_seconds` is not above 0, which includes NaN.
pub(crate) fn steering(world: &mut World, step_seconds: f32) {
    // Guards callers inside this crate, which can pass any step. A step of 0 would divide the
    // offset by 0 and give every entity an infinite or NaN velocity
    assert!(step_seconds > 0.0, "the step must be longer than 0 seconds");
    for (velocity, position, target, speed) in
        world.query_mut::<(&mut Velocity, &Position, &Target, &Speed)>()
    {
        let offset = target.0 - position.0;
        velocity.0 = (offset / step_seconds).clamp_length_max(speed.0);
    }
}

/// Turns each bone in `Bones` by `POSE_FRACTION` of the way towards its rotation in `PoseGoal`, and
/// sets the bone to that rotation once it lies within `SNAP_ANGLE` of it.
// Directs a hand model to a given pose
pub(crate) fn posing(world: &mut World, fraction: f32) {
    for (bones, goal) in world.query_mut::<(&mut Bones, &PoseGoal)>() {
        // Pairs each bone with its goal rotation
        for (bone, &goal_rotation) in bones.0.iter_mut().zip(&goal.0) {
            let turned = bone.slerp(goal_rotation, fraction);
            let remaining = turned.angle_between(goal_rotation);
            // Snaps the bone onto its goal once the bone is close
            *bone = if remaining < SNAP_ANGLE {
                goal_rotation
            } else {
                turned
            };
        }
    }
}

/// Copies the next pose in `idle_poses` into `PoseGoal` for each entity whose bones have all
/// reached their goal, and records that pose in `Idle`. Wraps round to the first pose after the
/// last, and leaves an entity whose `Idle` is `None` alone.
// Moves a hand model through a set of idle poses
pub(crate) fn idling(world: &mut World, idle_poses: &[Pose]) {
    for (idle, goal, bones) in world.query_mut::<(&mut Idle, &mut PoseGoal, &Bones)>() {
        // Skips an entity whose idle cycle is paused by a held key
        let Some(current) = idle.0 else {
            continue;
        };
        // Skips an entity whose bones have not all snapped onto the goal yet
        if bones.0 != goal.0 {
            continue;
        }

        let next = (current + 1) % idle_poses.len();
        idle.0 = Some(next);
        // Copies the next pose into the goal, reusing the goal's allocation
        goal.0.clone_from(&idle_poses[next].0);
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
