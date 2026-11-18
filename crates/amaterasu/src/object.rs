use wgpu::{BindGroup, Buffer, IndexFormat, Queue, RenderPass};
use crate::uniform::ObjectUniforms;

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

    pub(crate) fn draw(&self, render_pass: &mut RenderPass<'_>) {
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}