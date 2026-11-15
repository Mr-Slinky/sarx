//! The shape that the renderer draws, stored as a list of vertices and a list of indices.
//!
//! A vertex is one corner of a shape. An index is a position in the list of vertices. The GPU
//! draws the shape by joining every three indices into a triangle, whose corners are the vertices
//! that those indices pick out.

use bytemuck::{Pod, Zeroable};
use hermes::MeshData;
use std::mem::offset_of;
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 3], // the corner's position, in the model's own space
    normal: [f32; 3],   // the direction that the surface faces at this corner
}

/// The vertices and indices of one shape.
///
/// Each vertex stores a position in the model's own space, and the normal, which is the direction
/// that the surface faces at that corner.
///
/// A caller copies the vertices into a vertex buffer, using the bytes that `cast_vertex_bytes`
/// returns. It copies the indices into an index buffer in the same way, using `cast_index_bytes`.
/// The pipeline then reads each vertex back through `LAYOUT`.
///
/// # Examples
///
/// The example copies the hand's mesh into a vertex buffer and an index buffer, then draws every
/// triangle in it.
///
/// ```ignore
/// // Copies the vertices into a vertex buffer, using the bytes that `cast_vertex_bytes` returns
/// let mesh = Mesh::from_mesh_data(hand);
/// let descriptor = BufferInitDescriptor {
///     label: Some("Mesh vertices"),
///     contents: mesh.cast_vertex_bytes(),
///     usage: BufferUsages::VERTEX,
/// };
/// let vertex_buffer = device.create_buffer_init(&descriptor);
///
/// // Copies the indices into an index buffer, using the bytes that `cast_index_bytes` returns
/// let descriptor = BufferInitDescriptor {
///     label: Some("Mesh indices"),
///     contents: mesh.cast_index_bytes(),
///     usage: BufferUsages::INDEX,
/// };
/// let index_buffer = device.create_buffer_init(&descriptor);
///
/// // Draws every triangle, reading each vertex back through `LAYOUT`
/// render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
/// render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
/// render_pass.draw_indexed(0..mesh.count_indices(), 0, 0..1);
/// ```
pub(crate) struct Mesh {
    vertices: Vec<Vertex>, // every corner, each stored once
    indices: Vec<u32>,     // three entries for each triangle, each one an index into `vertices`
}

impl Mesh {
    /// The layout that tells the pipeline how to read the bytes that `cast_vertex_bytes` returns.
    ///
    /// Each vertex takes 24 bytes. The vertex shader receives the position at location 0 and the
    /// normal at location 1, each as three `f32` values.
    pub(crate) const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        // Steps 24 bytes from one vertex to the next
        array_stride: size_of::<Vertex>() as BufferAddress,
        // Moves to the next vertex for each vertex that the draw reads
        step_mode: VertexStepMode::Vertex,
        // Describes the two attributes inside each vertex
        attributes: &[
            // Hands the position to the shader at location 0
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: offset_of!(Vertex, position) as BufferAddress,
                shader_location: 0,
            },
            // Hands the normal to the shader at location 1
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: offset_of!(Vertex, normal) as BufferAddress,
                shader_location: 1,
            },
        ],
    };

    pub(crate) fn from_mesh_data(data: MeshData) -> Self {
        // Pairs each position with the normal at the same index, and builds one vertex from each pair
        let vertices = data
            .positions
            .into_iter()
            .zip(data.normals)
            .map(|(position, normal)| Vertex { position, normal })
            .collect();
        // Keeps the triangle list as it is, moving it out of `data` rather than copying it
        Self {
            vertices,
            indices: data.indices,
        }
    }

    pub(crate) fn count_indices(&self) -> u32 {
        self.indices.len() as u32
    }

    pub(crate) fn cast_vertex_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    pub(crate) fn cast_index_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }
}
