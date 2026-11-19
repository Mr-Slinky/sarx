use glam::Quat;

#[derive(Clone, Debug, PartialEq)]
pub struct Pose(pub Vec<Quat>); // one rotation for every bone, relative to its parent

pub struct HandPoses {
    pub rest: Pose,         // the pose that the hand starts in
    pub idle: Vec<Pose>,    // the poses that the hand cycles through, in order
    pub actions: Vec<Pose>, // the poses that some maintained input can hold the hand in
}