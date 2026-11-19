use crate::HandPoses;
use crate::components::{Bones, Idle, PoseGoal, Position, Speed, Target, Velocity};
use crate::random::Random;
use crate::systems::{idling, movement, posing, steering};
use enki::{Entity, World};
use glam::{Quat, Vec3};
use std::time::Duration;

pub struct Simulation {
    world: World,
    hand: Entity,
    // Stores the time that has passed and that no step has used yet. Between calls to `advance`,
    // it stays below `STEP`.
    unsimulated: Duration,
    poses: HandPoses,
    random: Random,
}

impl Simulation {
    // Defines the length of one step as one sixtieth of a second, which gives 60 steps a second.
    pub const STEP: Duration = Duration::from_nanos(16_666_667);
    // Defines the most elapsed time that one call to `advance` adds to `unsimulated`.
    const MAX_ELAPSED: Duration = Duration::from_millis(250);
    pub const HOVER_HEIGHT: f32 = 1.0;
    const HAND_SPEED: f32 = 50.0;
    const RANDOM_SEED: u32 = 0x9E37_79B9;

    pub fn new(poses: HandPoses) -> Self {
        assert!(
            !poses.idle.is_empty(),
            "the hand needs at least one idle pose"
        );
        let rest_bone_count = poses.rest.0.len();
        let mut every_pose = poses.idle.iter().chain(&poses.actions);
        let all_match = every_pose.all(|pose| pose.0.len() == rest_bone_count);
        // Guards `posing`, which pairs each bone with one goal rotation
        assert!(
            all_match,
            "every pose must have one rotation for each bone of the rest pose"
        );

        let mut world = World::new();
        let hand = world.spawn_empty();
        let start = Vec3::new(0.0, Self::HOVER_HEIGHT, 0.0);
        let components = (
            Position(start),
            Velocity(Vec3::ZERO),
            Target(start),
            Speed(Self::HAND_SPEED),
            Bones(poses.rest.0.clone()),
            PoseGoal(poses.idle[0].0.clone()),
            Idle(Some(0)),
        );

        world
            .add_components(hand, components)
            .expect("the hand should exist");
        // Returns the simulation, with no time left over
        Self {
            world,
            hand,
            unsimulated: Duration::ZERO,
            poses,
            random: Random::new(Self::RANDOM_SEED),
        }
    }

    pub fn hold_pose(&mut self, action: usize) {
        let pose = &self.poses.actions[action];
        let goal = self
            .world
            .get_mut::<PoseGoal>(self.hand)
            .expect("the hand always has a pose goal");
        goal.0.clone_from(&pose.0);
        let idle = self
            .world
            .get_mut::<Idle>(self.hand)
            .expect("the hand always has an idle cycle");
        // Pauses the idle cycle, which makes `idling` leave the goal alone
        idle.0 = None;
    }

    pub fn release_pose(&mut self) {
        let next = self.random.pick_below(self.poses.idle.len());
        let pose = &self.poses.idle[next];
        let goal = self
            .world
            .get_mut::<PoseGoal>(self.hand)
            .expect("the hand always has a pose goal");
        goal.0.clone_from(&pose.0);
        let idle = self
            .world
            .get_mut::<Idle>(self.hand)
            .expect("the hand always has an idle cycle");
        // Resumes the idle cycle from that pose
        idle.0 = Some(next);
    }

    pub fn find_bone_rotations(&self) -> &[Quat] {
        let bones = self
            .world
            .get::<Bones>(self.hand)
            .expect("the hand always has bones");
        &bones.0
    }

    pub fn set_hand_target(&mut self, point: Vec3) {
        let target = self
            .world
            .get_mut::<Target>(self.hand)
            .expect("the hand always has a target");
        target.0 = point;
    }

    /// Returns the hand's position
    pub fn hand_position(&self) -> Vec3 {
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
        // Runs before `idling`, which then sees on the same step whether the bones have arrived
        posing(&mut self.world);
        idling(&mut self.world, &self.poses.idle);
    }
}

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pose::Pose;

    /// Sets the largest difference at which two floats count as equal
    const TOLERANCE: f32 = 1e-4;

    /// Returns poses for a hand with one bone, which rests unturned and idles through two turns
    fn build_poses() -> HandPoses {
        HandPoses {
            rest: Pose(vec![Quat::IDENTITY]),
            idle: vec![
                Pose(vec![Quat::from_rotation_x(0.5)]),
                Pose(vec![Quat::from_rotation_x(-0.5)]),
            ],
            actions: vec![Pose(vec![Quat::from_rotation_y(1.0)])],
        }
    }

    /// Returns the target of the hand
    fn read_target(simulation: &Simulation) -> Vec3 {
        let target = simulation.world.get::<Target>(simulation.hand);
        target.expect("the hand has a target").0
    }

    /// Returns the furthest that the hand can move in one step
    fn find_reach() -> f32 {
        Simulation::HAND_SPEED * Simulation::STEP.as_secs_f32()
    }

    /// Checks that the time left over after each call to `advance` matches the expected time, and
    /// stays below one step after every call
    #[test]
    fn advance_with_elapsed_times_leaves_expected_time_unsimulated() {
        let step = Simulation::STEP;
        let half_step = step / 2;
        let one_ns = Duration::from_nanos(1);
        let one_ns_short = step - one_ns;
        let left_at_cap = Duration::from_nanos(16_666_662);
        let cases: [(&str, &[Duration], Duration); 8] = [
            ("one step", &[step], Duration::ZERO),
            (
                "two half steps",
                &[half_step, step - half_step],
                Duration::ZERO,
            ),
            ("no time", &[Duration::ZERO], Duration::ZERO),
            ("one ns short of a step", &[one_ns_short], one_ns_short),
            (
                "one ns short, then one ns",
                &[one_ns_short, one_ns],
                Duration::ZERO,
            ),
            ("exactly the cap", &[Simulation::MAX_ELAPSED], left_at_cap),
            ("largest duration", &[Duration::MAX], left_at_cap),
            (
                "largest duration twice",
                &[Duration::MAX, Duration::MAX],
                Duration::from_nanos(16_666_657),
            ),
        ];

        for (name, elapsed_times, expected) in cases {
            let mut simulation = Simulation::new(build_poses());
            let mut leftovers = Vec::new();

            for &elapsed in elapsed_times {
                simulation.advance(elapsed);
                leftovers.push(simulation.unsimulated);
            }

            let all_below_step = leftovers.iter().all(|&leftover| leftover < step);
            assert!(all_below_step, "leftovers for {name}: {leftovers:?}");
            assert_eq!(
                simulation.unsimulated, expected,
                "unsimulated time for {name}"
            );
        }
    }

    /// Checks that an elapsed time equal to the cap runs 14 steps, since 15 steps come to 5 ns
    /// more than the cap. A hand chasing a far target moves its full reach on each step, and the
    /// distance it covers therefore counts the steps
    #[test]
    fn advance_with_elapsed_at_cap_runs_fourteen_steps() {
        let mut simulation = Simulation::new(build_poses());
        simulation.set_hand_target(Vec3::new(1000.0, 0.0, 0.0));
        let start = simulation.hand_position();

        simulation.advance(Simulation::MAX_ELAPSED);

        let distance = simulation.hand_position().distance(start);
        let expected_distance = 14.0 * find_reach();
        assert!(
            (distance - expected_distance).abs() < TOLERANCE,
            "distance {distance}, expected {expected_distance}"
        );
        assert_eq!(simulation.unsimulated, Duration::from_nanos(16_666_662));
    }

    /// Checks that a new simulation holds a still hand at the hover height above the origin,
    /// aiming at where it already is, with no time left over
    #[test]
    fn new_with_hand_poses_starts_hand_at_rest_above_origin() {
        let simulation = Simulation::new(build_poses());

        let start = Vec3::new(0.0, Simulation::HOVER_HEIGHT, 0.0);
        let world = &simulation.world;
        let velocity = world
            .get::<Velocity>(simulation.hand)
            .map(|velocity| velocity.0);
        let speed = world.get::<Speed>(simulation.hand).map(|speed| speed.0);
        assert_eq!(simulation.hand_position(), start, "position");
        assert_eq!(velocity, Some(Vec3::ZERO), "velocity");
        assert_eq!(read_target(&simulation), start, "target");
        assert_eq!(speed, Some(Simulation::HAND_SPEED), "speed");
        assert_eq!(simulation.unsimulated, Duration::ZERO, "unsimulated time");
    }

    /// Checks that the target is the point given, and that the hand does not move until the
    /// simulation advances
    #[test]
    fn set_hand_target_with_points_stores_point_as_target() {
        let cases = [
            ("origin", Vec3::ZERO),
            ("positive point", Vec3::new(3.0, 0.0, 4.0)),
            ("negative point", Vec3::new(-7.5, 0.0, -2.25)),
            ("point above the ground", Vec3::new(1.0, 2.0, 1.0)),
            ("far point", Vec3::new(1e6, 0.0, -1e6)),
        ];

        for (name, point) in cases {
            let mut simulation = Simulation::new(build_poses());
            let start = simulation.hand_position();

            simulation.set_hand_target(point);

            assert_eq!(read_target(&simulation), point, "target for {name}");
            assert_eq!(simulation.hand_position(), start, "position for {name}");
        }
    }

    /// Checks that the position returned is the one stored on the hand
    #[test]
    fn find_hand_position_with_moved_hand_returns_stored_position() {
        let mut simulation = Simulation::new(build_poses());
        let moved = Vec3::new(2.0, 1.0, -3.0);
        simulation
            .world
            .get_mut::<Position>(simulation.hand)
            .expect("the hand has a position")
            .0 = moved;

        let position = simulation.hand_position();

        assert_eq!(position, moved);
    }

    /// Checks that one step never moves the hand further than its speed allows, whether the
    /// target is near or far
    #[test]
    fn advance_with_one_step_moves_hand_at_most_reach() {
        let cases = [
            ("near target", Vec3::new(0.1, 0.0, 0.0)),
            ("far target", Vec3::new(50.0, 0.0, 0.0)),
            ("far diagonal target", Vec3::new(-30.0, 0.0, 40.0)),
            ("target below the hand", Vec3::new(0.0, -5.0, 0.0)),
        ];

        for (name, ground_point) in cases {
            let mut simulation = Simulation::new(build_poses());
            simulation.set_hand_target(ground_point);
            let start = simulation.hand_position();

            simulation.advance(Simulation::STEP);

            let distance = simulation.hand_position().distance(start);
            let reach = find_reach();
            assert!(
                distance <= reach + TOLERANCE,
                "distance for {name}: {distance}"
            );
        }
    }

    /// Checks that the hand closes on its target every step, never passes it, and comes to rest
    /// on it. The target lies 5 units away, which takes 15 steps at full speed
    #[test]
    fn advance_with_enough_steps_reaches_target_without_overshoot() {
        let mut simulation = Simulation::new(build_poses());
        simulation.set_hand_target(Vec3::new(3.0, Simulation::HOVER_HEIGHT, 4.0));
        let target = read_target(&simulation);
        let start = simulation.hand_position();
        let mut positions = vec![start];

        for _ in 0..30 {
            simulation.advance(Simulation::STEP);
            positions.push(simulation.hand_position());
        }

        // Measures each position along the line from the start to the target, where the target
        // lies at the full distance and anything past it is an overshoot
        let direction = (target - start).normalize();
        let full_distance = start.distance(target);
        let progress: Vec<f32> = positions
            .iter()
            .map(|&position| (position - start).dot(direction))
            .collect();
        let never_back = progress
            .windows(2)
            .all(|pair| pair[1] >= pair[0] - TOLERANCE);
        let never_past = progress
            .iter()
            .all(|&covered| covered <= full_distance + TOLERANCE);
        let position = simulation.hand_position();
        assert!(never_back, "progress towards the target: {progress:?}");
        assert!(never_past, "progress past the target: {progress:?}");
        assert!(
            position.abs_diff_eq(target, TOLERANCE),
            "final position {position}"
        );
    }
}
