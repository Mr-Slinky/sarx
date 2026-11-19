//! Reads the poses of a model, each of which the file stores as an animation with one keyframe.

use crate::GltfError;
use crate::model::find_buffer;
use crate::skin::find_bone_nodes;
use gltf::animation::Property;
use gltf::animation::util::ReadOutputs;
use gltf::{Animation, Gltf};

/// Stores one pose: the name of its animation, and a rotation for each bone that the pose turns.
///
/// Entry `i` of `rotations` belongs to entry `i` of [`SkinData::bones`](crate::SkinData::bones).
/// Each rotation is a quaternion in the order `[x, y, z, w]`, relative to the bone's parent node,
/// and takes the place of the bone's rotation at rest. The example on
/// [`read_model`](crate::read_model) reads a pose.
pub struct PoseData {
    pub name: String,
    pub rotations: Vec<Option<[f32; 4]>>, // one rotation for each bone, or `None` where the pose leaves it out
}

/// Reads `animation` as a pose, from the block of bytes `binary`. `bone_nodes` lists the node
/// number of each bone, in bone order.
///
/// The function reads the rotation channels of the animation, and skips every other channel.
/// Returns `Ok(None)` if a rotation channel has more or fewer than one keyframe, since the
/// animation is then not a pose.
///
/// # Errors
///
/// Returns [`GltfError::InvalidPose`] if the animation rotates a node that is not a bone, or the
/// `gltf` crate cannot reach the rotations of a channel.
fn read_pose(
    animation: &Animation,
    bone_nodes: &[usize],
    binary: &[u8],
) -> Result<Option<PoseData>, GltfError> {
    // Starts with no rotation for every bone
    let mut rotations = vec![None; bone_nodes.len()];
    for channel in animation.channels() {
        let target = channel.target();
        if target.property() != Property::Rotation {
            // Skips a channel that changes a translation or a scale
            continue;
        }

        let node = target.node().index();
        // Finds the bone number of that node, or rejects the animation if the node is not a bone
        let Some(bone) = bone_nodes.iter().position(|&bone_node| bone_node == node) else {
            return Err(GltfError::InvalidPose(animation.index()));
        };

        let reader = channel.reader(|buffer| find_buffer(buffer, binary));
        // Reads the keyframe values, or rejects the animation if they are not rotations
        let Some(ReadOutputs::Rotations(keyframes)) = reader.read_outputs() else {
            return Err(GltfError::InvalidPose(animation.index()));
        };

        let keyframes: Vec<[f32; 4]> = keyframes.into_f32().collect();
        // Takes the single keyframe, or leaves the animation out if it has more or fewer
        let [rotation] = keyframes.as_slice() else {
            return Ok(None);
        };

        // Records the bone's rotation in this pose
        rotations[bone] = Some(*rotation);
    }

    // Copies the animation's name, or an empty name if the file gives none
    let name = animation.name().unwrap_or_default().to_owned();
    // Returns the pose
    Ok(Some(PoseData { name, rotations }))
}

/// Reads every animation in `gltf` that is a pose, from the block of bytes `binary`, in the order
/// that the file stores them.
///
/// # Errors
///
/// Returns [`GltfError::MissingSkin`] if the file has no skin. Returns [`GltfError::InvalidPose`]
/// if an animation rotates a node that is not a bone, or the `gltf` crate cannot reach the
/// rotations of a channel.
pub(crate) fn read_poses(gltf: &Gltf, binary: &[u8]) -> Result<Vec<PoseData>, GltfError> {
    // Lists the node number of each bone, in bone order
    let bone_nodes = find_bone_nodes(gltf)?;
    // Starts with no poses
    let mut poses = Vec::new();
    // Visits every animation in the file
    for animation in gltf.animations() {
        // Reads the animation as a pose, which is `None` if it has more than one keyframe
        if let Some(pose) = read_pose(&animation, &bone_nodes, binary)? {
            // Keeps the pose
            poses.push(pose);
        }
    }

    // Returns every pose, in the order that the file stores them
    Ok(poses)
}