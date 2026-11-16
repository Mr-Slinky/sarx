use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct SceneUniforms {
    pub(crate) view_projection: Mat4,
    pub(crate) light_direction: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ObjectUniforms {
    pub(crate) model: Mat4,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    /// Checks that `SceneUniforms` takes the 80 bytes of the `Scene` struct in `lit.wgsl`: a
    /// `mat4x4<f32>` of 64 bytes, then a `vec4<f32>` of 16
    #[test]
    fn size_of_with_scene_uniforms_returns_80() {
        let size = size_of::<SceneUniforms>();

        assert_eq!(size, 80, "size of `SceneUniforms`");
    }

    /// Checks that `light_direction` starts at byte 64, where `lit.wgsl` reads it
    #[test]
    fn offset_of_with_light_direction_returns_64() {
        let offset = offset_of!(SceneUniforms, light_direction);

        assert_eq!(offset, 64, "offset of `light_direction`");
    }

    /// Checks that `ObjectUniforms` takes the 64 bytes of the `Object` struct in `lit.wgsl`, one
    /// `mat4x4<f32>`
    #[test]
    fn size_of_with_object_uniforms_returns_64() {
        let size = size_of::<ObjectUniforms>();

        assert_eq!(size, 64, "size of `ObjectUniforms`");
    }
}
