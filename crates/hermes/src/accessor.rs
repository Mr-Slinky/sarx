//! The `componentType` codes in this module come from
//! [Accessor Data Types](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#accessor-data-types)
//! in the glTF 2.0 specification. `VEC3_SIZE` is derived from the specification rather than
//! stated in it

use crate::bytes::read_bytes_from;
use crate::GltfError;
use crate::document::{Accessor, Document};

const FLOAT_CODE: u32 = 5126;
const UNSIGNED_SHORT_CODE: u32 = 5123;
const UNSIGNED_INT_CODE: u32 = 5125;
const VEC3_SIZE: usize = 12;
const MIN_STRIDE: usize = 4;
const MAX_STRIDE: usize = 252;

/// Returns every vec3 element in the file, in the same order that the file stores them
pub(crate) fn read_vec3s(document: &Document, binary: &[u8], index: usize) -> Result<Vec<[f32; 3]>, GltfError> {
    // Finds the accessor, or fails where the index points past the end of the list
    let accessor = document.accessors.get(index).ok_or(GltfError::MissingAccessor(index))?;
    let is_vec3 = accessor.component_type == FLOAT_CODE && accessor.kind == "VEC3";
    if !is_vec3 {
        return Err(GltfError::UnsupportedAccessor(index));
    }

    // Rejects an accessor whose elements are all zero, since it has no data to read
    let view_index = accessor.buffer_view.ok_or(GltfError::UnsupportedAccessor(index))?;
    let layout = find_layout(document, accessor, view_index, VEC3_SIZE)?;
    ensure_fit(&layout, accessor.count, VEC3_SIZE, binary.len())?;
    let mut values = Vec::with_capacity(accessor.count);
    for element in 0..accessor.count {
        let offset = layout.start + element * layout.stride;
        let x = f32::from_le_bytes(read_bytes_from(binary, offset)?);
        let y = f32::from_le_bytes(read_bytes_from(binary, offset + 4)?);
        let z = f32::from_le_bytes(read_bytes_from(binary, offset + 8)?);
        values.push([x, y, z]);
    }

    Ok(values)
}

pub(crate) fn read_indices(document: &Document, binary: &[u8], index: usize) -> Result<Vec<u32>, GltfError> {
    let accessor = document.accessors.get(index).ok_or(GltfError::MissingAccessor(index))?;
    // Rejects every shape other than one number per element
    if accessor.kind != "SCALAR" {
        return Err(GltfError::UnsupportedAccessor(index));
    }

    let element_size = match accessor.component_type {
        // A 16-bit index takes two bytes
        UNSIGNED_SHORT_CODE => 2,
        // A 32-bit index takes four bytes
        UNSIGNED_INT_CODE => 4,
        // Every other type is an error
        _ => return Err(GltfError::UnsupportedAccessor(index)),
    };

    let view_index = accessor.buffer_view.ok_or(GltfError::UnsupportedAccessor(index))?;
    let layout = find_layout(document, accessor, view_index, element_size)?;
    // glTF forbids a stride on a buffer view that holds indices
    if layout.is_strided {
        return Err(GltfError::InvalidStride(view_index));
    }

    ensure_fit(&layout, accessor.count, element_size, binary.len())?;
    // Makes room for every index at once
    let mut indices = Vec::with_capacity(accessor.count);
    for element in 0..accessor.count {
        // Finds where this index starts
        let offset = layout.start + element * layout.stride;
        // Reads the index at its stored size, and widens a 16-bit index to 32 bits
        let value = if element_size == 2 {
            let short = u16::from_le_bytes(read_bytes_from(binary, offset)?);
            u32::from(short)
        } else {
            u32::from_le_bytes(read_bytes_from(binary, offset)?)
        };
        indices.push(value);
    }

    Ok(indices)
}

struct Layout {
    start: usize, // where the first element starts in the binary chunk
    stride: usize,
    view_end: usize,  // where the buffer view ends in the binary chunk
    is_strided: bool, // whether the buffer view states its own stride
}

fn find_layout(document: &Document, accessor: &Accessor, view_index: usize, element_size: usize) -> Result<Layout, GltfError> {
    let view = document.buffer_views.get(view_index).ok_or(GltfError::MissingBufferView(view_index))?;
    if let Some(stride) = view.byte_stride {
        // Applies the limits that glTF sets on `byteStride`
        let is_valid = (MIN_STRIDE..=MAX_STRIDE).contains(&stride) && stride % 4 == 0 && stride >= element_size;
        if !is_valid {
            return Err(GltfError::InvalidStride(view_index));
        }
    }

    let start = view.byte_offset.checked_add(accessor.byte_offset).ok_or(GltfError::Truncated)?;
    let view_end = view.byte_offset.checked_add(view.byte_length).ok_or(GltfError::Truncated)?;
    let stride = view.byte_stride.unwrap_or(element_size);
    let is_strided = view.byte_stride.is_some();
    Ok(Layout { start, stride, view_end, is_strided })
}

/// Checks that every element lies inside both the buffer view and the binary chunk, before a
/// caller reserves room for them.
fn ensure_fit(layout: &Layout, count: usize, element_size: usize, binary_len: usize) -> Result<(), GltfError> {
    // An empty accessor reads nothing, no matter where it starts
    let Some(last) = count.checked_sub(1) else {
        return Ok(());
    };

    let end = last
        .checked_mul(layout.stride)
        .and_then(|span| span.checked_add(layout.start))
        .and_then(|last_start| last_start.checked_add(element_size))
        .ok_or(GltfError::Truncated)?;
    if end > layout.view_end || end > binary_len {
        return Err(GltfError::Truncated);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::split_glb;

    /// Holds the GLB file that Blender exported for the hand model
    const HAND_MODEL: &[u8] =
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/hand-model.glb"));

    /// Returns the document whose JSON lists the given accessors and buffer views, and no meshes
    fn parse_document(accessors: &str, buffer_views: &str) -> Document {
        let json = format!(
            r#"{{"meshes": [], "accessors": [{accessors}], "bufferViews": [{buffer_views}]}}"#
        );
        serde_json::from_str(&json).expect("the test JSON is valid")
    }

    /// Returns the JSON of one accessor on buffer view 0, with no byte offset
    fn write_accessor(component_type: u32, count: usize, kind: &str) -> String {
        format!(
            r#"{{"bufferView": 0, "componentType": {component_type}, "count": {count}, "type": "{kind}"}}"#
        )
    }

    /// Returns the JSON of one buffer view with no stride, which starts at `byte_offset` in the
    /// binary chunk and holds `byte_length` bytes
    fn write_view(byte_offset: usize, byte_length: usize) -> String {
        format!(r#"{{"byteOffset": {byte_offset}, "byteLength": {byte_length}}}"#)
    }

    /// Returns the little-endian bytes of each float, one after another
    fn write_f32s(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// Returns the little-endian bytes of each 16-bit integer, one after another
    fn write_u16s(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// Returns the little-endian bytes of each 32-bit integer, one after another
    fn write_u32s(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// Returns the bits of each float in each element, so that NaN compares equal to itself
    fn read_bits(values: &[[f32; 3]]) -> Vec<[u32; 3]> {
        values.iter().map(|value| value.map(f32::to_bits)).collect()
    }

    /// Returns the document and the binary chunk of the hand model
    fn load_hand_model() -> (Document, &'static [u8]) {
        let glb = split_glb(HAND_MODEL).expect("the hand model is a valid GLB file");
        let document = serde_json::from_slice(glb.json).expect("the hand model has valid JSON");
        (document, glb.binary)
    }

    /// Checks that a tightly packed view gives every vec3, in the order that it is stored
    #[test]
    fn read_vec3s_with_packed_data_returns_every_element() {
        let document = parse_document(&write_accessor(5126, 3, "VEC3"), &write_view(0, 36));
        let binary = write_f32s(&[1.0, 2.0, 3.0, -4.5, 5.25, -6.0, 0.0, 0.5, 100.0]);

        let result = read_vec3s(&document, &binary, 0);

        let expected = vec![[1.0, 2.0, 3.0], [-4.5, 5.25, -6.0], [0.0, 0.5, 100.0]];
        assert!(matches!(result, Ok(values) if values == expected), "vec3s from a packed view");
    }

    /// Checks that the first element starts after the view's offset and the accessor's together
    #[test]
    fn read_vec3s_with_view_and_accessor_offsets_skips_both() {
        let document = parse_document(
            r#"{"bufferView": 0, "byteOffset": 4, "componentType": 5126, "count": 2, "type": "VEC3"}"#,
            &write_view(8, 28),
        );
        // Twelve bytes of padding come before the data, and four bytes after it. Each padding float
        // differs, so that a start in the wrong place reads different values
        let binary = write_f32s(&[-1.0, -2.0, -3.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, -4.0]);

        let result = read_vec3s(&document, &binary, 0);

        let expected = vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        assert!(matches!(result, Ok(values) if values == expected), "vec3s after both offsets");
    }

    /// Checks that two accessors on one interleaved view each read their own half of every vertex
    #[test]
    fn read_vec3s_with_interleaved_view_returns_positions_and_normals() {
        let document = parse_document(
            r#"{"bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3"},
               {"bufferView": 0, "byteOffset": 12, "componentType": 5126, "count": 2, "type": "VEC3"}"#,
            r#"{"byteLength": 48, "byteStride": 24}"#,
        );
        // Each vertex holds a position, then a normal
        let binary = write_f32s(&[1.0, 2.0, 3.0, 0.0, 1.0, 0.0, 4.0, 5.0, 6.0, 0.0, 0.0, -1.0]);

        let positions = read_vec3s(&document, &binary, 0);
        let normals = read_vec3s(&document, &binary, 1);

        let expected_positions = vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let expected_normals = vec![[0.0, 1.0, 0.0], [0.0, 0.0, -1.0]];
        assert!(
            matches!(positions, Ok(values) if values == expected_positions),
            "interleaved positions"
        );
        assert!(
            matches!(normals, Ok(values) if values == expected_normals),
            "interleaved normals"
        );
    }

    /// Checks that an accessor with no elements gives an empty list, even from an empty chunk
    #[test]
    fn read_vec3s_with_zero_count_returns_empty() {
        let document = parse_document(&write_accessor(5126, 0, "VEC3"), &write_view(0, 12));
        let cases = [
            ("empty chunk", Vec::new()),
            ("chunk with data", write_f32s(&[1.0, 2.0, 3.0])),
        ];

        for (name, binary) in cases {
            let result = read_vec3s(&document, &binary, 0);

            assert!(matches!(result, Ok(values) if values.is_empty()), "vec3s for {name}");
        }
    }

    /// Checks that an element whose last byte is the last byte of the chunk is read
    #[test]
    fn read_vec3s_with_data_ending_at_chunk_end_returns_last_element() {
        // The two offsets differ, so that adding either one twice reads from the wrong place
        let document = parse_document(
            r#"{"bufferView": 0, "byteOffset": 8, "componentType": 5126, "count": 1, "type": "VEC3"}"#,
            &write_view(16, 20),
        );
        let binary = write_f32s(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);

        let result = read_vec3s(&document, &binary, 0);

        let expected = vec![[7.0, 8.0, 9.0]];
        assert!(matches!(result, Ok(values) if values == expected), "vec3 at the end of the chunk");
    }

    /// Checks that special floats come back with exactly the bits that the file stores
    #[test]
    fn read_vec3s_with_special_floats_returns_exact_bits() {
        let document = parse_document(&write_accessor(5126, 3, "VEC3"), &write_view(0, 36));
        let smallest_subnormal = f32::from_bits(1);
        let stored = [
            [-0.0, f32::INFINITY, f32::NEG_INFINITY],
            [smallest_subnormal, f32::MIN_POSITIVE, f32::MAX],
            [f32::NAN, f32::MIN, f32::EPSILON],
        ];
        let binary = write_f32s(stored.as_flattened());

        let result = read_vec3s(&document, &binary, 0);

        let bits = result.map(|values| read_bits(&values));
        let expected = read_bits(&stored);
        assert!(matches!(bits, Ok(bits) if bits == expected), "bits of the special floats");
    }

    /// Checks that an index past the end of the accessor list fails with that index
    #[test]
    fn read_vec3s_with_accessor_past_end_returns_missing_accessor() {
        let view = write_view(0, 12);
        let one_accessor = parse_document(&write_accessor(5126, 1, "VEC3"), &view);
        let no_accessors = parse_document("", &view);
        let binary = write_f32s(&[1.0, 2.0, 3.0]);
        let cases = [
            ("one past the end", &one_accessor, 1),
            ("far past the end", &one_accessor, 7),
            ("largest index", &one_accessor, usize::MAX),
            ("no accessors", &no_accessors, 0),
        ];

        for (name, document, index) in cases {
            let result = read_vec3s(document, &binary, index);

            assert!(
                matches!(result, Err(GltfError::MissingAccessor(missing)) if missing == index),
                "error for {name}"
            );
        }
    }

    /// Checks that an accessor whose buffer view is past the end of the list fails with that view
    #[test]
    fn read_vec3s_with_buffer_view_past_end_returns_missing_buffer_view() {
        let binary = write_f32s(&[1.0, 2.0, 3.0]);
        let one_view = write_view(0, 12);
        let cases = [
            ("one past the end", one_view.as_str(), 1),
            ("largest index", one_view.as_str(), usize::MAX),
            ("no views", "", 0),
        ];

        for (name, views, view) in cases {
            let accessor = format!(
                r#"{{"bufferView": {view}, "componentType": 5126, "count": 1, "type": "VEC3"}}"#
            );
            let document = parse_document(&accessor, views);

            let result = read_vec3s(&document, &binary, 0);

            assert!(
                matches!(result, Err(GltfError::MissingBufferView(missing)) if missing == view),
                "error for {name}"
            );
        }
    }

    /// Checks that an accessor which is not a float vec3, or which has no buffer view, is rejected
    #[test]
    fn read_vec3s_with_unsupported_accessor_returns_unsupported_accessor() {
        let binary = write_f32s(&[0.0; 16]);
        let cases = [
            ("float VEC2", write_accessor(5126, 1, "VEC2")),
            ("float VEC4", write_accessor(5126, 1, "VEC4")),
            ("float SCALAR", write_accessor(5126, 1, "SCALAR")),
            ("float MAT3", write_accessor(5126, 1, "MAT3")),
            ("lower-case vec3", write_accessor(5126, 1, "vec3")),
            ("empty type", write_accessor(5126, 1, "")),
            ("signed byte VEC3", write_accessor(5120, 1, "VEC3")),
            ("unsigned byte VEC3", write_accessor(5121, 1, "VEC3")),
            ("signed short VEC3", write_accessor(5122, 1, "VEC3")),
            ("unsigned short VEC3", write_accessor(5123, 1, "VEC3")),
            ("unsigned int VEC3", write_accessor(5125, 1, "VEC3")),
            ("unknown code VEC3", write_accessor(0, 1, "VEC3")),
            (
                "no buffer view",
                r#"{"componentType": 5126, "count": 1, "type": "VEC3"}"#.to_string(),
            ),
        ];

        for (name, rejected) in cases {
            // A valid accessor comes first, so that the rejected one has the index 1
            let valid = write_accessor(5126, 1, "VEC3");
            let document = parse_document(&format!("{valid}, {rejected}"), &write_view(0, 12));

            let result = read_vec3s(&document, &binary, 1);

            assert!(matches!(result, Err(GltfError::UnsupportedAccessor(1))), "error for {name}");
        }
    }

    /// Checks that a buffer view whose stated stride glTF forbids for a vec3 fails with that view
    #[test]
    fn read_vec3s_with_invalid_stride_returns_invalid_stride() {
        let binary = vec![0; 64];
        let cases = [
            ("stride below the smallest", 2),
            ("stride not a multiple of four", 13),
            ("stride above the largest", 256),
            ("stride smaller than a vec3", 8),
            ("largest stride", usize::MAX),
        ];

        for (name, stride) in cases {
            // A valid view comes first, so that the rejected one has the index 1
            let accessor = r#"{"bufferView": 1, "componentType": 5126, "count": 1, "type": "VEC3"}"#;
            let views = format!(r#"{{"byteLength": 12}}, {{"byteLength": 12, "byteStride": {stride}}}"#);
            let document = parse_document(accessor, &views);

            let result = read_vec3s(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::InvalidStride(1))), "error for {name}");
        }
    }

    /// Checks that an accessor whose data runs past the end of its buffer view or of the binary
    /// chunk fails
    #[test]
    fn read_vec3s_with_data_past_view_or_chunk_end_returns_truncated() {
        // Each case gives the view offset, view length, accessor offset, stride, count and chunk
        // length. Except where the case names the view, the view holds the data and the chunk
        // cuts it short
        let cases = [
            ("chunk one byte short", 0, 36, 0, None, 3, 35),
            ("count one too many", 0, 48, 0, None, 4, 36),
            ("view offset at the end", 36, 12, 0, None, 1, 36),
            ("accessor offset one byte past", 0, 37, 25, None, 1, 36),
            ("stride past the end", 0, 44, 0, Some(16), 3, 36),
            ("empty chunk", 0, 12, 0, None, 1, 0),
            ("view one byte short", 0, 35, 0, None, 3, 36),
        ];

        for (name, view_offset, view_length, accessor_offset, stride, count, length) in cases {
            let accessor = format!(
                r#"{{"bufferView": 0, "byteOffset": {accessor_offset}, "componentType": 5126, "count": {count}, "type": "VEC3"}}"#
            );
            let view = match stride {
                Some(stride) => format!(
                    r#"{{"byteOffset": {view_offset}, "byteLength": {view_length}, "byteStride": {stride}}}"#
                ),
                None => write_view(view_offset, view_length),
            };
            let document = parse_document(&accessor, &view);
            let binary = vec![0; length];

            let result = read_vec3s(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that offsets whose sum overflows a `usize` fail as truncated, rather than panic
    #[test]
    fn read_vec3s_with_largest_offsets_returns_truncated() {
        let cases = [
            ("largest view offset", usize::MAX, 1),
            ("largest accessor offset", 1, usize::MAX),
        ];

        for (name, view_offset, accessor_offset) in cases {
            let accessor = format!(
                r#"{{"bufferView": 0, "byteOffset": {accessor_offset}, "componentType": 5126, "count": 1, "type": "VEC3"}}"#
            );
            let document = parse_document(&accessor, &write_view(view_offset, 36));
            let binary = vec![0; 36];

            let result = read_vec3s(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that a count far beyond what the chunk holds fails as truncated, before any allocation
    #[test]
    fn read_vec3s_with_largest_count_returns_truncated() {
        let document = parse_document(&write_accessor(5126, usize::MAX, "VEC3"), &write_view(0, 36));
        let binary = vec![0; 36];

        let result = read_vec3s(&document, &binary, 0);

        assert!(matches!(result, Err(GltfError::Truncated)), "error for the largest count");
    }

    /// Checks that the hand model's positions fill exactly the box that their accessor's `min` and
    /// `max` state, and start with the first position that the file stores
    #[test]
    fn read_vec3s_with_hand_model_positions_returns_values_filling_bounds() {
        let (document, binary) = load_hand_model();

        let positions = read_vec3s(&document, binary, 0).expect("the positions are readable");

        let min: [f32; 3] = std::array::from_fn(|axis| {
            positions.iter().map(|position| position[axis]).fold(f32::INFINITY, f32::min)
        });
        let max: [f32; 3] = std::array::from_fn(|axis| {
            positions.iter().map(|position| position[axis]).fold(f32::NEG_INFINITY, f32::max)
        });
        // The expected bounds are the accessor's `min` and `max` in the file's JSON, as `f32`
        assert_eq!(positions.len(), 3558, "number of positions");
        assert_eq!(min, [-0.78021425, -0.29307237, -3.9975772], "smallest value on each axis");
        assert_eq!(max, [1.3557904, 0.4874626, 0.012371443], "largest value on each axis");
        assert_eq!(positions[0], [-0.76153266, 0.15828243, -3.4236743], "first position");
    }

    /// Checks that every normal in the hand model has a length of one
    #[test]
    fn read_vec3s_with_hand_model_normals_returns_unit_vectors() {
        let (document, binary) = load_hand_model();

        let normals = read_vec3s(&document, binary, 1).expect("the normals are readable");

        let lengths: Vec<f32> =
            normals.iter().map(|[x, y, z]| (x * x + y * y + z * z).sqrt()).collect();
        assert_eq!(normals.len(), 3558, "number of normals");
        assert!(
            lengths.iter().all(|length| (length - 1.0).abs() < 1e-5),
            "lengths of the normals"
        );
    }

    /// Checks that the hand model's accessors other than float vec3s are rejected
    #[test]
    fn read_vec3s_with_hand_model_other_accessors_returns_unsupported_accessor() {
        let (document, binary) = load_hand_model();
        let cases = [
            ("texture coordinates", 2),
            ("joints", 3),
            ("weights", 4),
            ("indices", 5),
        ];

        for (name, index) in cases {
            let result = read_vec3s(&document, binary, index);

            assert!(
                matches!(result, Err(GltfError::UnsupportedAccessor(rejected)) if rejected == index),
                "error for {name}"
            );
        }
    }

    /// Checks that 16-bit indices come back widened to 32 bits, in the order that they are stored
    #[test]
    fn read_indices_with_u16_indices_returns_widened_values() {
        let document = parse_document(&write_accessor(5123, 6, "SCALAR"), &write_view(0, 12));
        let binary = write_u16s(&[0, 1, 2, 2, 1, 300]);

        let result = read_indices(&document, &binary, 0);

        let expected = vec![0, 1, 2, 2, 1, 300];
        assert!(matches!(result, Ok(indices) if indices == expected), "16-bit indices");
    }

    /// Checks that 32-bit indices come back unchanged, in the order that they are stored
    #[test]
    fn read_indices_with_u32_indices_returns_values() {
        let document = parse_document(&write_accessor(5125, 3, "SCALAR"), &write_view(0, 12));
        let binary = write_u32s(&[70_000, 3, 1_000_000]);

        let result = read_indices(&document, &binary, 0);

        let expected = vec![70_000, 3, 1_000_000];
        assert!(matches!(result, Ok(indices) if indices == expected), "32-bit indices");
    }

    /// Checks that the first index starts after the view's offset and the accessor's together
    #[test]
    fn read_indices_with_view_and_accessor_offsets_skips_both() {
        // Each case gives the component type, view offset, view length, accessor offset and chunk.
        // Three padding indices come before the indices 4, 5 and 6, and one after them. The padding
        // values differ, and so do the two offsets, so that a start in the wrong place reads other
        // values
        let cases = [
            ("16-bit", 5123, 4, 8, 2, write_u16s(&[10, 11, 12, 4, 5, 6, 13])),
            ("32-bit", 5125, 8, 16, 4, write_u32s(&[10, 11, 12, 4, 5, 6, 13])),
        ];

        for (name, component_type, view_offset, view_length, accessor_offset, binary) in cases {
            let accessor = format!(
                r#"{{"bufferView": 0, "byteOffset": {accessor_offset}, "componentType": {component_type}, "count": 3, "type": "SCALAR"}}"#
            );
            let document = parse_document(&accessor, &write_view(view_offset, view_length));

            let result = read_indices(&document, &binary, 0);

            assert!(
                matches!(result, Ok(indices) if indices == vec![4, 5, 6]),
                "{name} indices after both offsets"
            );
        }
    }

    /// Checks that an accessor with no elements gives an empty list, even from an empty chunk
    #[test]
    fn read_indices_with_zero_count_returns_empty() {
        let cases = [("16-bit", 5123), ("32-bit", 5125)];

        for (name, component_type) in cases {
            let accessor = write_accessor(component_type, 0, "SCALAR");
            let document = parse_document(&accessor, &write_view(0, 4));

            let result = read_indices(&document, &[], 0);

            assert!(
                matches!(result, Ok(indices) if indices.is_empty()),
                "{name} indices from an empty chunk"
            );
        }
    }

    /// Checks that the smallest and largest values of each index size come back unchanged
    #[test]
    fn read_indices_with_extreme_values_returns_exact_values() {
        let cases = [
            ("16-bit", 5123, write_u16s(&[0, 1, u16::MAX]), vec![0, 1, u32::from(u16::MAX)]),
            ("32-bit", 5125, write_u32s(&[0, 65_536, u32::MAX]), vec![0, 65_536, u32::MAX]),
        ];

        for (name, component_type, binary, expected) in cases {
            let accessor = write_accessor(component_type, 3, "SCALAR");
            let document = parse_document(&accessor, &write_view(0, binary.len()));

            let result = read_indices(&document, &binary, 0);

            assert!(matches!(result, Ok(indices) if indices == expected), "{name} extreme indices");
        }
    }

    /// Checks that an index whose last byte is the last byte of the chunk is read
    #[test]
    fn read_indices_with_data_ending_at_chunk_end_returns_last_index() {
        // Each case gives the component type, view offset, view length and chunk
        let cases = [
            ("16-bit", 5123, 4, 2, write_u16s(&[0, 0, 42])),
            ("32-bit", 5125, 8, 4, write_u32s(&[0, 0, 42])),
        ];

        for (name, component_type, view_offset, view_length, binary) in cases {
            let accessor = write_accessor(component_type, 1, "SCALAR");
            let document = parse_document(&accessor, &write_view(view_offset, view_length));

            let result = read_indices(&document, &binary, 0);

            assert!(
                matches!(result, Ok(indices) if indices == vec![42]),
                "{name} index at the end of the chunk"
            );
        }
    }

    /// Checks that an index past the end of the accessor list fails with that index
    #[test]
    fn read_indices_with_accessor_past_end_returns_missing_accessor() {
        let view = write_view(0, 2);
        let one_accessor = parse_document(&write_accessor(5123, 1, "SCALAR"), &view);
        let no_accessors = parse_document("", &view);
        let binary = write_u16s(&[0]);
        let cases = [
            ("one past the end", &one_accessor, 1),
            ("largest index", &one_accessor, usize::MAX),
            ("no accessors", &no_accessors, 0),
        ];

        for (name, document, index) in cases {
            let result = read_indices(document, &binary, index);

            assert!(
                matches!(result, Err(GltfError::MissingAccessor(missing)) if missing == index),
                "error for {name}"
            );
        }
    }

    /// Checks that an accessor whose buffer view is past the end of the list fails with that view
    #[test]
    fn read_indices_with_buffer_view_past_end_returns_missing_buffer_view() {
        let binary = write_u32s(&[0]);
        let one_view = write_view(0, 4);
        let cases = [
            ("one past the end", one_view.as_str(), 1),
            ("largest index", one_view.as_str(), usize::MAX),
            ("no views", "", 0),
        ];

        for (name, views, view) in cases {
            let accessor = format!(
                r#"{{"bufferView": {view}, "componentType": 5125, "count": 1, "type": "SCALAR"}}"#
            );
            let document = parse_document(&accessor, views);

            let result = read_indices(&document, &binary, 0);

            assert!(
                matches!(result, Err(GltfError::MissingBufferView(missing)) if missing == view),
                "error for {name}"
            );
        }
    }

    /// Checks that an accessor which is not a scalar of unsigned shorts or unsigned ints, or which
    /// has no buffer view, is rejected
    #[test]
    fn read_indices_with_unsupported_accessor_returns_unsupported_accessor() {
        let binary = write_u32s(&[0; 16]);
        let cases = [
            ("unsigned short VEC3", write_accessor(5123, 1, "VEC3")),
            ("unsigned int VEC3", write_accessor(5125, 1, "VEC3")),
            ("unsigned short VEC2", write_accessor(5123, 1, "VEC2")),
            ("lower-case scalar", write_accessor(5123, 1, "scalar")),
            ("empty type", write_accessor(5123, 1, "")),
            ("signed byte SCALAR", write_accessor(5120, 1, "SCALAR")),
            ("unsigned byte SCALAR", write_accessor(5121, 1, "SCALAR")),
            ("signed short SCALAR", write_accessor(5122, 1, "SCALAR")),
            ("float SCALAR", write_accessor(5126, 1, "SCALAR")),
            ("unknown code SCALAR", write_accessor(0, 1, "SCALAR")),
            (
                "no buffer view",
                r#"{"componentType": 5125, "count": 1, "type": "SCALAR"}"#.to_string(),
            ),
        ];

        for (name, rejected) in cases {
            // A valid accessor comes first, so that the rejected one has the index 1
            let valid = write_accessor(5125, 1, "SCALAR");
            let document = parse_document(&format!("{valid}, {rejected}"), &write_view(0, 4));

            let result = read_indices(&document, &binary, 1);

            assert!(matches!(result, Err(GltfError::UnsupportedAccessor(1))), "error for {name}");
        }
    }

    /// Checks that a buffer view of indices which states any stride fails with that view, since
    /// glTF forbids a stride on indices
    #[test]
    fn read_indices_with_any_stride_returns_invalid_stride() {
        let binary = vec![0; 64];
        // Each case gives the component type and the stride
        let cases = [
            ("16-bit, stride of four", 5123, 4),
            ("32-bit, stride of four", 5125, 4),
            ("32-bit, largest valid vertex stride", 5125, 252),
            ("32-bit, largest stride", 5125, usize::MAX),
        ];

        for (name, component_type, stride) in cases {
            // A valid view comes first, so that the rejected one has the index 1
            let accessor = format!(
                r#"{{"bufferView": 1, "componentType": {component_type}, "count": 1, "type": "SCALAR"}}"#
            );
            let views = format!(r#"{{"byteLength": 4}}, {{"byteLength": 4, "byteStride": {stride}}}"#);
            let document = parse_document(&accessor, &views);

            let result = read_indices(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::InvalidStride(1))), "error for {name}");
        }
    }

    /// Checks that an accessor whose indices run past the end of its buffer view or of the binary
    /// chunk fails
    #[test]
    fn read_indices_with_data_past_view_or_chunk_end_returns_truncated() {
        // Each case gives the component type, view offset, view length, accessor offset, count and
        // chunk length. Except where the case names the view, the view holds the indices and the
        // chunk cuts them short
        let cases = [
            ("16-bit chunk one byte short", 5123, 0, 6, 0, 3, 5),
            ("32-bit chunk one byte short", 5125, 0, 12, 0, 3, 11),
            ("16-bit count one too many", 5123, 0, 8, 0, 4, 6),
            ("32-bit view offset at the end", 5125, 12, 4, 0, 1, 12),
            ("32-bit accessor offset one byte past", 5125, 0, 13, 9, 1, 12),
            ("empty chunk", 5123, 0, 2, 0, 1, 0),
            ("32-bit view one byte short", 5125, 0, 11, 0, 3, 12),
        ];

        for (name, component_type, view_offset, view_length, accessor_offset, count, length) in cases {
            let accessor = format!(
                r#"{{"bufferView": 0, "byteOffset": {accessor_offset}, "componentType": {component_type}, "count": {count}, "type": "SCALAR"}}"#
            );
            let document = parse_document(&accessor, &write_view(view_offset, view_length));
            let binary = vec![0; length];

            let result = read_indices(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that offsets whose sum overflows a `usize` fail as truncated, rather than panic
    #[test]
    fn read_indices_with_largest_offsets_returns_truncated() {
        let cases = [
            ("16-bit largest view offset", 5123, usize::MAX, 1),
            ("32-bit largest accessor offset", 5125, 1, usize::MAX),
        ];

        for (name, component_type, view_offset, accessor_offset) in cases {
            let accessor = format!(
                r#"{{"bufferView": 0, "byteOffset": {accessor_offset}, "componentType": {component_type}, "count": 1, "type": "SCALAR"}}"#
            );
            let document = parse_document(&accessor, &write_view(view_offset, 12));
            let binary = vec![0; 12];

            let result = read_indices(&document, &binary, 0);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that a count far beyond what the chunk holds fails as truncated, before any allocation
    #[test]
    fn read_indices_with_largest_count_returns_truncated() {
        let document = parse_document(&write_accessor(5125, usize::MAX, "SCALAR"), &write_view(0, 12));
        let binary = vec![0; 12];

        let result = read_indices(&document, &binary, 0);

        assert!(matches!(result, Err(GltfError::Truncated)), "error for the largest count");
    }

    /// Checks that the hand model's indices form whole triangles over its 3558 vertices
    #[test]
    fn read_indices_with_hand_model_indices_returns_triangle_list() {
        let (document, binary) = load_hand_model();

        let indices = read_indices(&document, binary, 5).expect("the indices are readable");

        let largest = indices.iter().copied().max();
        assert_eq!(indices.len(), 19_788, "number of indices");
        assert_eq!(indices.len() % 3, 0, "remainder after whole triangles");
        assert_eq!(largest, Some(3557), "largest index");
        assert_eq!(indices[..6], [0, 960, 2738, 0, 2738, 961], "first two triangles");
    }

    /// Checks that the hand model's accessors other than scalar indices are rejected
    #[test]
    fn read_indices_with_hand_model_other_accessors_returns_unsupported_accessor() {
        let (document, binary) = load_hand_model();
        let cases = [
            ("positions", 0),
            ("normals", 1),
            ("texture coordinates", 2),
            ("joints", 3),
            ("weights", 4),
        ];

        for (name, index) in cases {
            let result = read_indices(&document, binary, index);

            assert!(
                matches!(result, Err(GltfError::UnsupportedAccessor(rejected)) if rejected == index),
                "error for {name}"
            );
        }
    }

    /// Checks that a layout starts after both offsets, steps by the stride or the element size, and
    /// ends where the buffer view ends
    #[test]
    fn find_layout_with_standard_and_extreme_views_returns_layout() {
        // Each case gives the views, the accessor's view and offset, the element size, then the
        // expected start, stride, view end and whether the view states a stride
        let cases = [
            ("no offsets or stride", r#"{"byteLength": 36}"#, 0, 0, 12, (0, 12, 36, false)),
            ("view offset", r#"{"byteOffset": 40, "byteLength": 36}"#, 0, 0, 12, (40, 12, 76, false)),
            ("accessor offset", r#"{"byteLength": 8}"#, 0, 6, 2, (6, 2, 8, false)),
            ("both offsets", r#"{"byteOffset": 40, "byteLength": 16}"#, 0, 6, 4, (46, 4, 56, false)),
            ("stride", r#"{"byteLength": 48, "byteStride": 24}"#, 0, 0, 12, (0, 24, 48, true)),
            (
                "stride and offsets",
                r#"{"byteOffset": 8, "byteLength": 64, "byteStride": 32}"#,
                0,
                12,
                12,
                (20, 32, 72, true),
            ),
            ("largest stride", r#"{"byteLength": 252, "byteStride": 252}"#, 0, 0, 12, (0, 252, 252, true)),
            (
                "second of two views",
                r#"{"byteOffset": 40, "byteLength": 60}, {"byteOffset": 100, "byteLength": 32, "byteStride": 16}"#,
                1,
                4,
                12,
                (104, 16, 132, true),
            ),
        ];

        for (name, views, view_index, accessor_offset, element_size, expected) in cases {
            let accessor = format!(
                r#"{{"bufferView": {view_index}, "byteOffset": {accessor_offset}, "componentType": 5126, "count": 1, "type": "VEC3"}}"#
            );
            let document = parse_document(&accessor, views);

            let layout = find_layout(&document, &document.accessors[0], view_index, element_size);

            let fields = layout.map(|layout| (layout.start, layout.stride, layout.view_end, layout.is_strided));
            assert!(matches!(fields, Ok(fields) if fields == expected), "layout for {name}");
        }
    }

    /// Checks that a layout for a buffer view past the end of the list fails with that view
    #[test]
    fn find_layout_with_buffer_view_past_end_returns_missing_buffer_view() {
        let document = parse_document(
            r#"{"bufferView": 2, "componentType": 5126, "count": 1, "type": "VEC3"}"#,
            &format!("{}, {}", write_view(0, 12), write_view(12, 12)),
        );

        let layout = find_layout(&document, &document.accessors[0], 2, 12);

        assert!(matches!(layout, Err(GltfError::MissingBufferView(2))), "error for view 2");
    }
}
