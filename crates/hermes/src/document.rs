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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::split_glb;

    /// Holds the GLB file that Blender exported for the hand model
    const HAND_MODEL: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/hand-model.glb"));

    /// Returns the JSON chunk of the hand model
    fn read_hand_json() -> &'static [u8] {
        let glb = split_glb(HAND_MODEL).expect("the hand model is a valid GLB file");
        glb.json
    }

    /// Returns the buffer view, byte offset, component type, count and kind of the accessor that
    /// `json` describes, or `None` where `json` fails to parse
    fn summarise_accessor(json: &str) -> Option<(usize, usize, u32, usize, String)> {
        let accessor = serde_json::from_str::<Accessor>(json).ok()?;
        Some((
            accessor.buffer_view,
            accessor.byte_offset,
            accessor.component_type,
            accessor.count,
            accessor.kind,
        ))
    }

    /// Returns the byte offset and byte stride of the buffer view that `json` describes, or `None`
    /// where `json` fails to parse
    fn summarise_buffer_view(json: &str) -> Option<(usize, Option<usize>)> {
        let view = serde_json::from_str::<BufferView>(json).ok()?;
        Some((view.byte_offset, view.byte_stride))
    }

    /// Returns the position and normal accessors that `json` names, or `None` where `json` fails
    /// to parse
    fn summarise_attributes(json: &str) -> Option<(usize, usize)> {
        let attributes = serde_json::from_str::<Attributes>(json).ok()?;
        Some((attributes.position, attributes.normal))
    }

    /// Returns the position, normal and index accessors that `json` names, or `None` where `json`
    /// fails to parse
    fn summarise_primitive(json: &str) -> Option<(usize, usize, usize)> {
        let primitive = serde_json::from_str::<Primitive>(json).ok()?;
        let attributes = primitive.attributes;
        Some((attributes.position, attributes.normal, primitive.indices))
    }

    /// Returns the number of meshes, accessors and buffer views in the document that `json`
    /// describes, or `None` where `json` fails to parse
    fn summarise_document(json: &str) -> Option<(usize, usize, usize)> {
        let document = serde_json::from_str::<Document>(json).ok()?;
        Some((
            document.meshes.len(),
            document.accessors.len(),
            document.buffer_views.len(),
        ))
    }

    /// Checks that the hand model's document holds one mesh, whose primitive names the expected
    /// accessors, along with 115 accessors and 115 buffer views
    #[test]
    fn document_from_json_with_hand_model_returns_mesh_and_lists() {
        let json = read_hand_json();

        let document = serde_json::from_slice::<Document>(json).expect("the hand model's JSON parses");

        let primitive_counts: Vec<usize> = document.meshes.iter().map(|mesh| mesh.primitives.len()).collect();
        let primitive = &document.meshes[0].primitives[0];
        assert_eq!(primitive_counts, [1], "primitives in each mesh");
        assert_eq!(primitive.attributes.position, 0, "position accessor");
        assert_eq!(primitive.attributes.normal, 1, "normal accessor");
        assert_eq!(primitive.indices, 5, "index accessor");
        assert_eq!(document.accessors.len(), 115, "number of accessors");
        assert_eq!(document.buffer_views.len(), 115, "number of buffer views");
    }

    /// Checks that the hand model's first six accessors keep their buffer view, offset, number
    /// type, count and shape
    #[test]
    fn document_from_json_with_hand_model_returns_mesh_accessors() {
        let json = read_hand_json();
        let expected = [
            (0, 0, 5126, 3558, "VEC3"),
            (1, 0, 5126, 3558, "VEC3"),
            (2, 0, 5126, 3558, "VEC2"),
            (3, 0, 5121, 3558, "VEC4"),
            (4, 0, 5126, 3558, "VEC4"),
            (5, 0, 5123, 19788, "SCALAR"),
        ];

        let document = serde_json::from_slice::<Document>(json).expect("the hand model's JSON parses");

        for (index, expected) in expected.into_iter().enumerate() {
            let accessor = &document.accessors[index];
            let fields = (
                accessor.buffer_view,
                accessor.byte_offset,
                accessor.component_type,
                accessor.count,
                accessor.kind.as_str(),
            );
            assert_eq!(fields, expected, "fields of accessor {index}");
        }
    }

    /// Checks that the hand model's first six buffer views keep their byte offsets, and have no
    /// stride
    #[test]
    fn document_from_json_with_hand_model_returns_buffer_view_offsets() {
        let json = read_hand_json();
        let expected = [0, 42696, 85392, 113856, 128088, 185016];

        let document = serde_json::from_slice::<Document>(json).expect("the hand model's JSON parses");

        for (index, expected_offset) in expected.into_iter().enumerate() {
            let view = &document.buffer_views[index];
            let fields = (view.byte_offset, view.byte_stride);
            assert_eq!(fields, (expected_offset, None), "fields of buffer view {index}");
        }
    }

    /// Checks that a document parses from standard and extreme JSON, ignoring the fields that it
    /// does not model
    #[test]
    fn document_from_json_with_standard_and_extreme_json_returns_lists() {
        let cases = [
            (
                "one of each",
                r#"{
                    "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 2 }] }],
                    "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }],
                    "bufferViews": [{ "buffer": 0, "byteLength": 36 }]
                }"#,
                (1, 1, 1),
            ),
            (
                "empty lists",
                r#"{ "meshes": [], "accessors": [], "bufferViews": [] }"#,
                (0, 0, 0),
            ),
            (
                "mesh with no primitives",
                r#"{ "meshes": [{ "primitives": [] }], "accessors": [], "bufferViews": [] }"#,
                (1, 0, 0),
            ),
            (
                "unknown fields at the top level",
                r#"{
                    "asset": { "version": "2.0" }, "scene": 0, "scenes": [{ "nodes": [0] }],
                    "nodes": [{ "mesh": 0 }], "skins": [], "animations": [], "buffers": [{ "byteLength": 0 }],
                    "extensionsUsed": ["KHR_materials_unlit"],
                    "meshes": [], "accessors": [], "bufferViews": []
                }"#,
                (0, 0, 0),
            ),
            (
                "unknown fields in each mesh",
                r#"{
                    "meshes": [{ "name": "Hand", "weights": [0.5], "primitives": [] }],
                    "accessors": [], "bufferViews": []
                }"#,
                (1, 0, 0),
            ),
            (
                "space padding after the JSON, as a GLB chunk adds",
                "{ \"meshes\": [], \"accessors\": [], \"bufferViews\": [] }   ",
                (0, 0, 0),
            ),
            (
                "snake case buffer views beside the camel case list",
                r#"{ "meshes": [], "accessors": [], "bufferViews": [{}], "buffer_views": [{}, {}] }"#,
                (0, 0, 1),
            ),
        ];

        for (name, json, expected) in cases {
            let counts = summarise_document(json);

            assert_eq!(counts, Some(expected), "meshes, accessors and buffer views for {name}");
        }
    }

    /// Checks that a document missing a list parses with that list empty, since the glTF
    /// specification makes `meshes`, `accessors` and `bufferViews` optional
    #[test]
    fn document_from_json_with_absent_lists_returns_empty_lists() {
        let cases = [
            ("no meshes", r#"{ "accessors": [], "bufferViews": [] }"#),
            ("no accessors", r#"{ "meshes": [], "bufferViews": [] }"#),
            ("no buffer views", r#"{ "meshes": [], "accessors": [] }"#),
            ("asset alone", r#"{ "asset": { "version": "2.0" } }"#),
        ];

        for (name, json) in cases {
            let counts = summarise_document(json);

            assert_eq!(counts, Some((0, 0, 0)), "meshes, accessors and buffer views for {name}");
        }
    }

    /// Checks that a document is rejected where its JSON is malformed or has the wrong shape
    #[test]
    fn document_from_json_with_abnormal_json_returns_error() {
        let cases = [
            ("empty text", ""),
            ("unclosed object", r#"{ "meshes": [], "accessors": [], "bufferViews": ["#),
            ("trailing comma", r#"{ "meshes": [], "accessors": [], "bufferViews": [], }"#),
            ("top level number", "0"),
            ("top level text", r#""meshes""#),
            ("top level null", "null"),
            ("meshes as an object", r#"{ "meshes": {}, "accessors": [], "bufferViews": [] }"#),
            ("accessors as null", r#"{ "meshes": [], "accessors": null, "bufferViews": [] }"#),
            ("buffer views as a number", r#"{ "meshes": [], "accessors": [], "bufferViews": 0 }"#),
            ("mesh missing primitives", r#"{ "meshes": [{}], "accessors": [], "bufferViews": [] }"#),
            (
                "primitive missing indices",
                r#"{ "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 } }] }],
                     "accessors": [], "bufferViews": [] }"#,
            ),
            (
                "accessor missing count",
                r#"{ "meshes": [], "accessors": [{ "bufferView": 0, "componentType": 5126, "type": "VEC3" }],
                     "bufferViews": [] }"#,
            ),
            (
                "buffer view with a negative offset",
                r#"{ "meshes": [], "accessors": [], "bufferViews": [{ "byteOffset": -1 }] }"#,
            ),
        ];

        for (name, json) in cases {
            let result = serde_json::from_str::<Document>(json);

            assert!(result.is_err(), "document for {name} should be rejected");
        }
    }

    /// Checks that a primitive parses its attributes and index accessor, ignoring the fields that
    /// it does not model
    #[test]
    fn primitive_from_json_with_standard_and_extreme_json_returns_accessors() {
        let cases = [
            (
                "standard",
                r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 5 }"#,
                (0, 1, 5),
            ),
            (
                "unknown fields",
                r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 5, "mode": 4, "material": 0, "targets": [] }"#,
                (0, 1, 5),
            ),
            (
                "one accessor for every field",
                r#"{ "attributes": { "POSITION": 0, "NORMAL": 0 }, "indices": 0 }"#,
                (0, 0, 0),
            ),
        ];

        for (name, json, expected) in cases {
            let accessors = summarise_primitive(json);

            assert_eq!(accessors, Some(expected), "position, normal and index accessors for {name}");
        }
    }

    /// Checks that a primitive is rejected where a field it needs is missing or has the wrong type
    #[test]
    fn primitive_from_json_with_abnormal_json_returns_error() {
        let cases = [
            ("missing attributes", r#"{ "indices": 5 }"#),
            ("missing indices", r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 } }"#),
            ("attributes as a number", r#"{ "attributes": 0, "indices": 5 }"#),
            ("negative indices", r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": -1 }"#),
            ("indices as text", r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": "5" }"#),
            ("null indices", r#"{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": null }"#),
            ("attributes missing normal", r#"{ "attributes": { "POSITION": 0 }, "indices": 5 }"#),
        ];

        for (name, json) in cases {
            let accessors = summarise_primitive(json);

            assert_eq!(accessors, None, "primitive for {name} should be rejected");
        }
    }

    /// Checks that a primitive keeps the largest accessor index that a `usize` can hold, in every
    /// field
    #[test]
    fn primitive_from_json_with_largest_values_returns_accessors() {
        let json = format!(
            r#"{{ "attributes": {{ "POSITION": {max}, "NORMAL": {max} }}, "indices": {max} }}"#,
            max = usize::MAX,
        );

        let accessors = summarise_primitive(&json);

        let expected = (usize::MAX, usize::MAX, usize::MAX);
        assert_eq!(accessors, Some(expected), "position, normal and index accessors with the largest values");
    }

    /// Checks that the attributes parse from the upper case glTF names, ignoring every other
    /// attribute
    #[test]
    fn attributes_from_json_with_standard_and_extreme_json_returns_accessors() {
        let cases = [
            ("standard", r#"{ "POSITION": 0, "NORMAL": 1 }"#, (0, 1)),
            (
                "other attributes, as Blender exports",
                r#"{ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2, "JOINTS_0": 3, "WEIGHTS_0": 4 }"#,
                (0, 1),
            ),
            ("fields in reverse order", r#"{ "NORMAL": 7, "POSITION": 6 }"#, (6, 7)),
            ("one accessor for both", r#"{ "POSITION": 3, "NORMAL": 3 }"#, (3, 3)),
        ];

        for (name, json, expected) in cases {
            let accessors = summarise_attributes(json);

            assert_eq!(accessors, Some(expected), "position and normal accessors for {name}");
        }
    }

    /// Checks that the attributes are rejected where a name is missing, misspelt or holds the
    /// wrong type
    #[test]
    fn attributes_from_json_with_abnormal_json_returns_error() {
        let cases = [
            ("missing position", r#"{ "NORMAL": 1 }"#),
            ("missing normal", r#"{ "POSITION": 0 }"#),
            ("lower case names", r#"{ "position": 0, "normal": 1 }"#),
            ("negative position", r#"{ "POSITION": -1, "NORMAL": 1 }"#),
            ("fractional normal", r#"{ "POSITION": 0, "NORMAL": 1.5 }"#),
            ("position as text", r#"{ "POSITION": "0", "NORMAL": 1 }"#),
            ("null normal", r#"{ "POSITION": 0, "NORMAL": null }"#),
        ];

        for (name, json) in cases {
            let accessors = summarise_attributes(json);

            assert_eq!(accessors, None, "attributes for {name} should be rejected");
        }
    }

    /// Checks that an accessor parses from the camel case glTF names, with an absent byte offset
    /// read as 0 and unknown fields ignored
    #[test]
    fn accessor_from_json_with_standard_and_extreme_json_returns_fields() {
        let cases = [
            (
                "standard",
                r#"{ "bufferView": 1, "componentType": 5126, "count": 3558, "type": "VEC3" }"#,
                (1, 0, 5126, 3558, "VEC3"),
            ),
            (
                "explicit byte offset",
                r#"{ "bufferView": 1, "byteOffset": 24, "componentType": 5126, "count": 2, "type": "VEC3" }"#,
                (1, 24, 5126, 2, "VEC3"),
            ),
            (
                "explicit zero byte offset",
                r#"{ "bufferView": 0, "byteOffset": 0, "componentType": 5123, "count": 6, "type": "SCALAR" }"#,
                (0, 0, 5123, 6, "SCALAR"),
            ),
            (
                "unknown fields",
                r#"{ "bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3",
                     "min": [-1.0, -1.0, -1.0], "max": [1.0, 1.0, 1.0], "normalized": false, "name": "positions" }"#,
                (0, 0, 5126, 1, "VEC3"),
            ),
            (
                "zero count",
                r#"{ "bufferView": 0, "componentType": 5125, "count": 0, "type": "SCALAR" }"#,
                (0, 0, 5125, 0, "SCALAR"),
            ),
            (
                "kind outside the specification",
                r#"{ "bufferView": 0, "componentType": 1, "count": 1, "type": "" }"#,
                (0, 0, 1, 1, ""),
            ),
        ];

        for (name, json, (view, offset, component, count, kind)) in cases {
            let fields = summarise_accessor(json);

            let expected = (view, offset, component, count, kind.to_string());
            assert_eq!(fields, Some(expected), "fields of the accessor for {name}");
        }
    }

    /// Checks that an accessor keeps the largest value that each number field can hold
    #[test]
    fn accessor_from_json_with_largest_values_returns_fields() {
        let json = format!(
            r#"{{ "bufferView": {max}, "byteOffset": {max}, "componentType": {}, "count": {max}, "type": "MAT4" }}"#,
            u32::MAX,
            max = usize::MAX,
        );

        let fields = summarise_accessor(&json);

        let expected = (usize::MAX, usize::MAX, u32::MAX, usize::MAX, "MAT4".to_string());
        assert_eq!(fields, Some(expected), "fields of the accessor with the largest values");
    }

    /// Checks that an accessor is rejected where a number field holds one past its largest value
    #[test]
    fn accessor_from_json_with_values_past_largest_returns_error() {
        let past_usize = usize::MAX as u128 + 1;
        let past_u32 = u64::from(u32::MAX) + 1;
        let cases = [
            (
                "buffer view",
                format!(r#"{{ "bufferView": {past_usize}, "componentType": 5126, "count": 1, "type": "VEC3" }}"#),
            ),
            (
                "byte offset",
                format!(r#"{{ "bufferView": 0, "byteOffset": {past_usize}, "componentType": 5126, "count": 1, "type": "VEC3" }}"#),
            ),
            (
                "component type",
                format!(r#"{{ "bufferView": 0, "componentType": {past_u32}, "count": 1, "type": "VEC3" }}"#),
            ),
            (
                "count",
                format!(r#"{{ "bufferView": 0, "componentType": 5126, "count": {past_usize}, "type": "VEC3" }}"#),
            ),
        ];

        for (name, json) in cases {
            let fields = summarise_accessor(&json);

            assert_eq!(fields, None, "accessor with an overflowing {name} should be rejected");
        }
    }

    /// Checks that an accessor is rejected where a required field is missing, misnamed or holds
    /// the wrong type
    #[test]
    fn accessor_from_json_with_abnormal_json_returns_error() {
        let cases = [
            ("missing buffer view", r#"{ "componentType": 5126, "count": 1, "type": "VEC3" }"#),
            ("missing component type", r#"{ "bufferView": 0, "count": 1, "type": "VEC3" }"#),
            ("missing count", r#"{ "bufferView": 0, "componentType": 5126, "type": "VEC3" }"#),
            ("missing type", r#"{ "bufferView": 0, "componentType": 5126, "count": 1 }"#),
            (
                "snake case buffer view",
                r#"{ "buffer_view": 0, "componentType": 5126, "count": 1, "type": "VEC3" }"#,
            ),
            (
                "field named kind rather than type",
                r#"{ "bufferView": 0, "componentType": 5126, "count": 1, "kind": "VEC3" }"#,
            ),
            ("negative count", r#"{ "bufferView": 0, "componentType": 5126, "count": -1, "type": "VEC3" }"#),
            (
                "negative byte offset",
                r#"{ "bufferView": 0, "byteOffset": -4, "componentType": 5126, "count": 1, "type": "VEC3" }"#,
            ),
            ("fractional count", r#"{ "bufferView": 0, "componentType": 5126, "count": 1.5, "type": "VEC3" }"#),
            ("count as text", r#"{ "bufferView": 0, "componentType": 5126, "count": "1", "type": "VEC3" }"#),
            ("type as a number", r#"{ "bufferView": 0, "componentType": 5126, "count": 1, "type": 3 }"#),
            ("null buffer view", r#"{ "bufferView": null, "componentType": 5126, "count": 1, "type": "VEC3" }"#),
            (
                "null byte offset",
                r#"{ "bufferView": 0, "byteOffset": null, "componentType": 5126, "count": 1, "type": "VEC3" }"#,
            ),
            (
                "negative component type",
                r#"{ "bufferView": 0, "componentType": -5126, "count": 1, "type": "VEC3" }"#,
            ),
            ("number rather than object", "0"),
        ];

        for (name, json) in cases {
            let fields = summarise_accessor(json);

            assert_eq!(fields, None, "accessor for {name} should be rejected");
        }
    }

    /// Checks that a buffer view reads an absent byte offset as 0 and an absent stride as `None`,
    /// and keeps the values that the JSON gives
    #[test]
    fn buffer_view_from_json_with_standard_and_extreme_json_returns_fields() {
        let cases = [
            (
                "offset and unknown fields",
                r#"{ "buffer": 0, "byteOffset": 42696, "byteLength": 42696, "target": 34962 }"#,
                (42696, None),
            ),
            ("empty object", "{}", (0, None)),
            ("explicit zero offset", r#"{ "byteOffset": 0 }"#, (0, None)),
            ("explicit stride", r#"{ "byteOffset": 8, "byteStride": 32 }"#, (8, Some(32))),
            ("stride with no offset", r#"{ "byteStride": 12 }"#, (0, Some(12))),
            ("null stride", r#"{ "byteOffset": 4, "byteStride": null }"#, (4, None)),
        ];

        for (name, json, expected) in cases {
            let fields = summarise_buffer_view(json);

            assert_eq!(fields, Some(expected), "offset and stride of the buffer view for {name}");
        }
    }

    /// Checks that a buffer view keeps the largest offset and stride that a `usize` can hold
    #[test]
    fn buffer_view_from_json_with_largest_values_returns_fields() {
        let json = format!(r#"{{ "byteOffset": {max}, "byteStride": {max} }}"#, max = usize::MAX);

        let fields = summarise_buffer_view(&json);

        assert_eq!(fields, Some((usize::MAX, Some(usize::MAX))), "offset and stride with the largest values");
    }

    /// Checks that a buffer view is rejected where its offset or stride is out of range or holds
    /// the wrong type
    #[test]
    fn buffer_view_from_json_with_abnormal_json_returns_error() {
        let past_usize = usize::MAX as u128 + 1;
        let cases = [
            ("negative offset", r#"{ "byteOffset": -1 }"#.to_string()),
            ("negative stride", r#"{ "byteStride": -12 }"#.to_string()),
            ("fractional offset", r#"{ "byteOffset": 0.5 }"#.to_string()),
            ("stride as text", r#"{ "byteStride": "12" }"#.to_string()),
            ("null offset", r#"{ "byteOffset": null }"#.to_string()),
            ("overflowing offset", format!(r#"{{ "byteOffset": {past_usize} }}"#)),
            ("overflowing stride", format!(r#"{{ "byteStride": {past_usize} }}"#)),
            ("text rather than object", r#""0""#.to_string()),
        ];

        for (name, json) in cases {
            let fields = summarise_buffer_view(&json);

            assert_eq!(fields, None, "buffer view for {name} should be rejected");
        }
    }
}
