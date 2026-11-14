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
pub(crate) struct ObjectUnforms {
    pub(crate) model: Mat4,
}
