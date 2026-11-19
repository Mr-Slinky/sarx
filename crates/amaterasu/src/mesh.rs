//! The shape that the renderer draws, stored as a list of vertices and a list of indices.
//!
//! A vertex is one corner of a shape. An index is a position in the list of vertices. The GPU
//! draws the shape by joining every three indices into a triangle, whose corners are the vertices
//! that those indices pick out.

use bytemuck::{Pod, Zeroable};
use hermes::{MeshData, SkinData};
use std::mem::offset_of;
use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

pub(crate) trait GpuVertex: Pod {
    // Describes the bytes of one vertex
    const LAYOUT: VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 3], // the corner's position, in the model's own space
    normal: [f32; 3],   // the direction that the surface faces at this corner
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct SkinnedVertex {
    position: [f32; 3], // the vertex's position at rest, in the model's own space
    normal: [f32; 3],   // the direction that the surface faces at this vertex, at rest
    joints: [u16; 4],   // the four bones that move this vertex
    weights: [f32; 4],  // how strongly each of those bones pulls, adding up to 1
}

impl GpuVertex for Vertex {
    /// The layout that tells the pipeline how to read the bytes returned by `cast_vertex_bytes`.
    ///
    /// Each vertex takes 24 bytes. The vertex shader receives the position at location 0 and the
    /// normal at location 1, each as three `f32` values.
    const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
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
}


impl GpuVertex for SkinnedVertex {
    const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        // Steps 48 bytes from one vertex to the next
        array_stride: size_of::<SkinnedVertex>() as BufferAddress,
        // Moves to the next vertex for each vertex that the draw reads
        step_mode: VertexStepMode::Vertex,
        // Describes the four attributes inside each vertex
        attributes: &[
            // Hands the position to the shader at location 0
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: offset_of!(SkinnedVertex, position) as BufferAddress,
                shader_location: 0,
            },
            // Hands the normal to the shader at location 1
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: offset_of!(SkinnedVertex, normal) as BufferAddress,
                shader_location: 1,
            },
            // Hands the four bone numbers to the shader at location 2, as unsigned integers
            VertexAttribute {
                format: VertexFormat::Uint16x4,
                offset: offset_of!(SkinnedVertex, joints) as BufferAddress,
                shader_location: 2,
            },
            // Hands the four weights to the shader at location 3
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: offset_of!(SkinnedVertex, weights) as BufferAddress,
                shader_location: 3,
            },
        ],
    };
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
pub(crate) struct Mesh<V> {
    vertices: Vec<V>,  // every corner, each stored once
    indices: Vec<u32>, // three entries for each triangle, each one an index into `vertices`
}


impl <V: GpuVertex> Mesh<V> {
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

impl Mesh<Vertex> {

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

    pub(crate) fn build_ground(half_size: f32) -> Self {
        // Finds the edges of the square, `half_size` either side of the origin
        let near = half_size;
        let far = -half_size;
        // Describes a square at height 0, with every normal pointing straight up
        let data = MeshData {
            positions: vec![[far, 0.0, far], [far, 0.0, near], [near, 0.0, near], [near, 0.0, far]],
            normals: vec![[0.0, 1.0, 0.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        // Interleaves the positions and normals, as for any other mesh
        Self::from_mesh_data(data)
    }

}

impl Mesh<SkinnedVertex> {
    pub(crate) fn from_skinned_data(data: &MeshData, skin: &SkinData) -> Self {
        // Creates an empty list with room for every vertex
        let mut vertices = Vec::with_capacity(data.positions.len());
        // Builds each vertex in turn
        for (index, &position) in data.positions.iter().enumerate() {
            // Builds the vertex from the four values at the same index
            let vertex = SkinnedVertex {
                position,
                normal: data.normals[index],
                joints: skin.joints[index],
                weights: skin.weights[index],
            };
            // Adds the finished vertex
            vertices.push(vertex);
        }
        // Copies the triangle list, since `data` is only borrowed
        Self {
            vertices,
            indices: data.indices.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns one triangle, whose positions and normals all differ, so that a value read from the
    /// wrong place shows up
    fn build_triangle() -> MeshData {
        MeshData {
            positions: vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            normals: vec![[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
            indices: vec![0, 1, 2],
        }
    }

    /// Returns an empty mesh, with no positions, normals or indices
    fn build_empty() -> MeshData {
        MeshData {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        }
    }

    // Checks that the layout matches the `VertexInput` struct of `lit.wgsl`: a 24-byte stride,
    // with the position at offset 0 and location 0, and the normal at offset 12 and location 1
    #[test]
    fn layout_with_position_and_normal_matches_lit_shader() {
        let layout = Vertex::LAYOUT;

        let attributes = layout.attributes;

        assert_eq!(layout.array_stride, 24, "stride");
        assert!(
            matches!(layout.step_mode, VertexStepMode::Vertex),
            "step mode"
        );
        assert_eq!(attributes.len(), 2, "count of attributes");
        assert_eq!(attributes[0].offset, 0, "offset of the position");
        assert_eq!(attributes[0].shader_location, 0, "location of the position");
        assert_eq!(
            attributes[0].format,
            VertexFormat::Float32x3,
            "format of the position"
        );
        assert_eq!(attributes[1].offset, 12, "offset of the normal");
        assert_eq!(attributes[1].shader_location, 1, "location of the normal");
        assert_eq!(
            attributes[1].format,
            VertexFormat::Float32x3,
            "format of the normal"
        );
    }

    /// Checks that one triangle gives three vertices of 24 bytes each, and three indices
    #[test]
    fn from_mesh_data_with_triangle_returns_three_vertices_and_indices() {
        let data = build_triangle();

        let mesh = Mesh::from_mesh_data(data);

        assert_eq!(mesh.cast_vertex_bytes().len(), 72, "bytes of vertices");
        assert_eq!(mesh.count_indices(), 3, "count of indices");
    }

    /// Checks that an empty mesh gives no vertex bytes and no indices
    #[test]
    fn from_mesh_data_with_empty_data_returns_empty_mesh() {
        let data = build_empty();

        let mesh = Mesh::from_mesh_data(data);

        assert!(mesh.cast_vertex_bytes().is_empty(), "bytes of vertices");
        assert!(mesh.cast_index_bytes().is_empty(), "bytes of indices");
        assert_eq!(mesh.count_indices(), 0, "count of indices");
    }

    /// Checks that each vertex stores its position in its first 12 bytes and its normal in the 12
    /// after, as `LAYOUT` describes
    #[test]
    fn cast_vertex_bytes_with_triangle_returns_position_then_normal() {
        let mesh = Mesh::from_mesh_data(build_triangle());

        let bytes = mesh.cast_vertex_bytes();

        let first_position: [f32; 3] = bytemuck::pod_read_unaligned(&bytes[0..12]);
        let first_normal: [f32; 3] = bytemuck::pod_read_unaligned(&bytes[12..24]);
        let second_position: [f32; 3] = bytemuck::pod_read_unaligned(&bytes[24..36]);
        assert_eq!(first_position, [1.0, 2.0, 3.0], "first position");
        assert_eq!(first_normal, [0.0, 1.0, 0.0], "first normal");
        assert_eq!(second_position, [4.0, 5.0, 6.0], "second position");
    }

    /// Checks that the index bytes hold each index as four bytes, in order
    #[test]
    fn cast_index_bytes_with_triangle_returns_each_index_in_order() {
        let mesh = Mesh::from_mesh_data(build_triangle());

        let bytes = mesh.cast_index_bytes();

        let (chunks, _) = bytes.as_chunks::<4>();
        let indices: Vec<u32> = chunks
            .iter()
            .map(|chunk| u32::from_ne_bytes(*chunk))
            .collect();
        assert_eq!(bytes.len(), 12, "bytes of indices");
        assert_eq!(indices, [0, 1, 2], "indices read back");
    }

    /// Checks that the count of indices matches the length of the index list, whatever its length
    #[test]
    fn count_indices_with_index_lists_returns_length() {
        let cases = [
            ("no indices", Vec::new()),
            ("one triangle", vec![0, 1, 2]),
            ("two triangles", vec![0, 1, 2, 2, 1, 0]),
        ];

        for (name, indices) in cases {
            let expected = indices.len() as u32;
            let data = MeshData {
                indices,
                ..build_triangle()
            };
            let mesh = Mesh::from_mesh_data(data);

            let count = mesh.count_indices();

            assert_eq!(count, expected, "count of indices for {name}");
        }
    }
}
