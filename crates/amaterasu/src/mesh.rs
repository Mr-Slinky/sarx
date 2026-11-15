//! The shape that the renderer draws, stored as a list of vertices.
//!
//! A vertex is one corner of a shape. The GPU draws the shape from its vertices, joining every
//! three of them into a triangle.

use std::mem::offset_of;
use bytemuck::{Pod, Zeroable};
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};
use hermes::MeshData;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 3], // where the corner sits, in the model's own space
    normal: [f32; 3],   // the direction that the surface faces at this corner
}

/// The vertices of one shape, each stored as an x, y and z coordinate in clip space.
///
/// Clip space is the coordinate system that the GPU draws in. Its x runs from -1 at the window's
/// left edge to 1 at its right edge, and its y runs from -1 at the bottom edge to 1 at the top.
///
/// A caller copies the vertices into a vertex buffer, using the bytes that `to_bytes` returns.
/// The pipeline then reads each vertex back through `LAYOUT`.
///
/// # Examples
///
/// The example copies a triangle into a vertex buffer, then draws every vertex in it.
///
/// ```ignore
/// // Copies the vertices into a vertex buffer, using the bytes that `to_bytes` returns
/// let mesh = Mesh::build_triangle();
/// let contents = mesh.to_bytes();
/// let descriptor = BufferInitDescriptor {
///     label: Some("Mesh vertices"),
///     contents: &contents,
///     usage: BufferUsages::VERTEX,
/// };
/// let vertex_buffer = device.create_buffer_init(&descriptor);
///
/// // Draws every vertex in the buffer, which the pipeline reads back through `LAYOUT`
/// render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
/// render_pass.draw(0..mesh.vertex_count(), 0..1);
/// ```
pub(crate) struct Mesh {
    vertices: Vec<Vertex>, // every corner, each stored once
    indices: Vec<u32>,     // three entries for each triangle, each one a position in `vertices`
}

impl Mesh {
    /// The layout that tells the pipeline how to read the bytes that `to_bytes` returns.
    ///
    /// Each vertex takes 12 bytes. The vertex shader receives them at location 0, as three `f32`
    /// values.
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
        let vertices = data.positions
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
