use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

/// The vertices of one shape, each stored as an x, y and z coordinate in clip space.
pub(crate) struct Mesh {
    vertices: Vec<[f32; 3]>,
}

impl Mesh {
    /// Tells the GPU how to read the bytes that `to_bytes` returns: 12 bytes per vertex, passed
    /// to the vertex shader at location 0 as three `f32` values.
    pub(crate) const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: size_of::<[f32; 3]>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    };

    /// Returns a flat triangle, half the window's width and half its height, centred in the
    /// window.
    pub(crate) fn triangle() -> Self {
        Self {
            vertices: vec![
                [ 0.0,  0.5, 0.0], // top corner
                [-0.5, -0.5, 0.0], // bottom left corner
                [ 0.5, -0.5, 0.0], // bottom right corner
            ],
        }
    }

    /// Returns the number of vertices in the mesh.
    pub(crate) fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    /// Returns every coordinate of every vertex as bytes, in order, ready to copy into a vertex
    /// buffer.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        self.vertices
            .iter()
            .flatten()
            .flat_map(|coordinate| coordinate.to_ne_bytes())
            .collect()
    }
}