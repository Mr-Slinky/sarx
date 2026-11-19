use glam::{Mat4, Quat, Vec3};
use hermes::BoneData;

pub(crate) struct Skeleton {
    bones: Vec<SkeletonBone>,
}

impl Skeleton {
    pub(crate) fn from_bones(bones: &[BoneData]) -> Self {
        let bones = bones
            .iter()
            .map(|bone| SkeletonBone {
                parent: bone.parent,
                translation: Vec3::from_array(bone.translation),
                scale: Vec3::from_array(bone.scale),
                inverse_bind: Mat4::from_cols_array(&bone.inverse_bind),
            })
            .collect();

        Self { bones }
    }
    
    pub(crate) fn count_bones(&self) -> usize {
        self.bones.len()
    }

    pub(crate) fn build_joint_matrices(&self, rotations: &[Quat]) -> Vec<Mat4>{
        assert_eq!(rotations.len(), self.bones.len(), "each bone needs one rotation");
        let mut globals = Vec::with_capacity(self.bones.len());
        let mut joint_matrices = Vec::with_capacity(self.bones.len());
        for (bone, &rotation) in self.bones.iter().zip(rotations) {
            let local = Mat4::from_scale_rotation_translation(bone.scale, rotation, bone.translation);
            let global = match bone.parent {
                Some(parent) => globals[parent] * local,
                None => local,
            };
            globals.push(global);
            joint_matrices.push(global * bone.inverse_bind);
        }
        joint_matrices
    }
}

// ========================================================================================== \\
//                                       Helper Structs                                       \\
// ========================================================================================== \\
struct SkeletonBone {
    parent: Option<usize>, // the parent's position, or `None` for a root bone
    translation: Vec3,     // where the bone is placed relative to its parent
    scale: Vec3,
    inverse_bind: Mat4, // undoes the bone's place at rest
}
