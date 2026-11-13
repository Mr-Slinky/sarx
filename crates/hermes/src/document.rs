//! Models the JSON found in a .glb file to the structs that are defined in this module.
//! The parsed JSON looks like this:
//! ```json
//! {
//!   "asset": { "generator": "Khronos glTF Blender I/O v5.0.21", "version": "2.0" },
//!   "scene": 0,
//!
//!   "meshes": [
//!     {
//!       "name": "Hand  - Realistic",
//!       "primitives": [
//!         {
//!           "attributes": {
//!             "POSITION": 0,
//!             "NORMAL": 1,
//!             "TEXCOORD_0": 2,
//!             "JOINTS_0": 3,
//!             "WEIGHTS_0": 4
//!           },
//!           "indices": 5
//!         }
//!       ]
//!     }
//!   ],
//!
//!   "accessors": [
//!     { "bufferView": 0, "componentType": 5126, "count": 3558, "type": "VEC3",
//!       "min": [-0.780, -0.293, -3.998], "max": [1.356, 0.487, 0.012] },
//!     { "bufferView": 1, "componentType": 5126, "count": 3558,  "type": "VEC3" },
//!     { "bufferView": 2, "componentType": 5126, "count": 3558,  "type": "VEC2" },
//!     { "bufferView": 3, "componentType": 5121, "count": 3558,  "type": "VEC4" },
//!     { "bufferView": 4, "componentType": 5126, "count": 3558,  "type": "VEC4" },
//!     { "bufferView": 5, "componentType": 5123, "count": 19788, "type": "SCALAR" }
//!     // ... 109 more, used by the animations
//!   ],
//!
//!   "bufferViews": [
//!     { "buffer": 0, "byteOffset": 0,      "byteLength": 42696, "target": 34962 },
//!     { "buffer": 0, "byteOffset": 42696,  "byteLength": 42696, "target": 34962 },
//!     { "buffer": 0, "byteOffset": 85392,  "byteLength": 28464, "target": 34962 },
//!     { "buffer": 0, "byteOffset": 113856, "byteLength": 14232, "target": 34962 },
//!     { "buffer": 0, "byteOffset": 128088, "byteLength": 56928, "target": 34962 },
//!     { "buffer": 0, "byteOffset": 185016, "byteLength": 39576, "target": 34963 }
//!     // ... 109 more
//!   ],
//!
//!   "buffers": [ { "byteLength": 227716 } ],
//!
//!   "nodes":      [ /* 24: the forearm, the finger bones, the mesh */ ],
//!   "skins":      [ /* 1 */ ],
//!   "animations": [ /* 8 */ ]
//! }
//! ```
//!



use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Document {
    pub(crate) meshes: Vec<Mesh>,
    pub(crate) accessors: Vec<Accessor>, // how to read each kind of data
    pub(crate) buffer_views: Vec<BufferView>,
}

#[derive(Deserialize)]
pub(crate) struct Mesh {
    pub(crate) primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
pub(crate) struct Primitive {
    pub(crate) attributes: Attributes, // the accessor for each kind of vertex data
    pub(crate) indices: usize,         // the accessor for the triangle list
}

/// The individual data of a vertex
#[derive(Deserialize)]
pub(crate) struct Attributes {
    #[serde(rename = "POSITION")]
    pub(crate) position: usize, // the accessor for the vertex positions
    #[serde(rename = "NORMAL")]
    pub(crate) normal: usize,   // the accessor for the vertex normals
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Accessor {
    pub(crate) buffer_view: usize, // the `id` of the buffer
    #[serde(default)]
    pub(crate) byte_offset: usize,
    pub(crate) component_type: u32, // the number type, as a code that the glTF specification sets
    pub(crate) count: usize,
    #[serde(rename = "type")]
    pub(crate) kind: String, // the shape of each element, such as `VEC3` or `SCALAR`
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BufferView {
    #[serde(default)]
    pub(crate) byte_offset: usize,
    pub(crate) byte_stride: Option<usize>,
}
