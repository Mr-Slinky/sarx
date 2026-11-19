//! The GPU buffers of one object that the renderer draws, such as the hand or the ground.

use wgpu::{BindGroup, Buffer, IndexFormat, Queue, RenderPass};
use crate::uniform::ObjectUniforms;

/// The buffers that one mesh needs on the GPU, together with the bind group that hands its
/// uniforms to the shader.
///
/// [`GpuContext::create_render_object`](crate::context::GpuContext::create_render_object) builds
/// one from a mesh. Each frame, the renderer writes the object's uniforms with
/// [`write_uniforms`](Self::write_uniforms), then draws it with [`draw`](Self::draw).
pub(crate) struct RenderObject {
    pub(crate) vertex_buffer: Buffer,  // the object's vertices, in GPU memory
    pub(crate) index_buffer: Buffer,   // the object's triangle list, in GPU memory
    pub(crate) index_count: u32,       // the number of entries in `index_buffer`
    pub(crate) uniform_buffer: Buffer, // the object's model matrix and colour
    pub(crate) bind_group: BindGroup,  // hands `uniform_buffer` to the shader, at group 1
}

impl RenderObject {
    /// Copies the uniforms into the object's buffer, which the GPU reads at the next draw
    pub(crate) fn write_uniforms(&self, queue: &Queue, uniforms: &ObjectUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Records a draw of every triangle in the object into `render_pass`.
    ///
    /// Before the draw, the method points group 1 at the object's bind group, vertex buffer slot 0
    /// at its vertex buffer, and the index buffer at its triangle list. The caller selects the
    /// pipeline and sets group 0 first, as `Renderer::render` does.
    pub(crate) fn draw(&self, render_pass: &mut RenderPass<'_>) {
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}