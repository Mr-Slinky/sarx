//! The values that the shader reads from uniform buffers.
//!
//! A uniform buffer stores values that stay the same for every vertex and every pixel of a draw.
//! Each struct here matches a struct in `lit.wgsl` field for field. The renderer casts a struct to
//! bytes with `bytemuck`, and copies the bytes into the struct's uniform buffer.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};

/// The values that every object in a frame shares, which the shader reads at group 0.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct SceneUniforms {
    /// Turns a position in the world into a position on the screen, as
    /// [`Camera::build_view_projection`](crate::Camera::build_view_projection) builds it.
    pub(crate) view_projection: Mat4,
    /// The direction that the light travels in. The shader reads only the first three values.
    pub(crate) light_direction: Vec4,
}

/// The values for one object, which the shader reads at group 1.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ObjectUniforms {
    pub(crate) model: Mat4, // transformations such as rotation, translation and scaling
    pub(crate) colour: Vec4,
}
