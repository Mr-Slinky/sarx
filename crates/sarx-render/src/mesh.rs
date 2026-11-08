//! The shape that the renderer draws, stored as a list of vertices.
//!
//! A vertex is one corner of a shape. The GPU draws the shape from its vertices, joining every
//! three of them into a triangle.

use wgpu::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

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
    /// The x, y and z coordinates of each vertex, in drawing order.
    vertices: Vec<[f32; 3]>,
}

impl Mesh {
    /// The layout that tells the pipeline how to read the bytes that `to_bytes` returns.
    ///
    /// Each vertex takes 12 bytes. The vertex shader receives them at location 0, as three `f32`
    /// values.
    pub(crate) const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: size_of::<[f32; 3]>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: &[VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    };

    /// Returns a flat triangle in the centre of the window.
    ///
    /// The triangle has three vertices, one for each corner. It is half as wide as the window, and
    /// half as tall.
    ///
    /// # Examples
    ///
    /// The triangle has one vertex for each of its three corners.
    ///
    /// ```ignore
    /// let triangle = Mesh::build_triangle();
    /// assert_eq!(triangle.vertex_count(), 3); // one vertex for each corner
    /// ```
    pub(crate) fn build_triangle() -> Self {
        Self {
            vertices: vec![
                [0.0, 0.5, 0.0],   // top corner
                [-0.5, -0.5, 0.0], // bottom left corner
                [0.5, -0.5, 0.0],  // bottom right corner
            ],
        }
    }

    /// Returns the number of vertices in the mesh.
    ///
    /// # Examples
    ///
    /// A triangle has three vertices.
    ///
    /// ```ignore
    /// let triangle = Mesh::build_triangle();
    /// assert_eq!(triangle.vertex_count(), 3);
    /// ```
    pub(crate) fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    /// Returns every coordinate of every vertex as bytes, in order.
    ///
    /// A caller copies these bytes into a vertex buffer. Each coordinate takes four bytes, in the
    /// CPU's native byte order.
    ///
    /// # Examples
    ///
    /// The triangle comes to 36 bytes, starting with the x coordinate of its top corner.
    ///
    /// ```ignore
    /// // Three vertices of three coordinates, at four bytes each
    /// let bytes = Mesh::build_triangle().to_bytes();
    /// assert_eq!(bytes.len(), 36);
    ///
    /// // The first four bytes are the x coordinate of the top corner
    /// let top_x = 0.0_f32.to_ne_bytes();
    /// assert_eq!(bytes[0..4], top_x);
    /// ```
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        self.vertices
            .iter()
            .flatten()
            .flat_map(|coordinate| coordinate.to_ne_bytes())
            .collect()
    }
}
