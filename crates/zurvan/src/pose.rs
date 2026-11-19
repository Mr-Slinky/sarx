//! Defines the poses that the hand's bones turn towards.

use glam::Quat;

/// Gives each bone of the hand a rotation, with the bones in the same order in every pose.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose(pub Vec<Quat>); // one rotation for every bone, relative to its parent

/// Stores every pose of the hand, which [`Simulation::new`](crate::Simulation::new) takes.
///
/// The example on [`Simulation`](crate::Simulation) builds a set of poses for a hand with one bone.
pub struct HandPoses {
    pub rest: Pose,         // the pose that the hand starts in
    pub idle: Vec<Pose>,    // the poses that the hand cycles through, in order
    pub actions: Vec<Pose>, // the poses that some maintained input can hold the hand in
}