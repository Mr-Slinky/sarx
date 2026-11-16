//! Holds the fixtures and GLB builders that the tests of several modules share.

use crate::glb::{BINARY_CHUNK, HEADER_LENGTH, JSON_CHUNK, MAGIC, VERSION};

/// Contains the bytes of the GLB file that Blender exported for the hand model
pub(crate) const HAND_MODEL: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/hand-model.glb"));

/// Returns the 12-byte GLB header with the given `magic`, `version` and total `length`
///
/// ```text
/// build_header(MAGIC, 2, 28)  ->  the bytes of "glTF", then 2 and 28 as little-endian u32s
/// ```
pub(crate) fn build_header(magic: u32, version: u32, length: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LENGTH);
    header.extend_from_slice(&magic.to_le_bytes());
    header.extend_from_slice(&version.to_le_bytes());
    header.extend_from_slice(&length.to_le_bytes());
    header
}

/// Appends a chunk whose header states `length`, whatever the true length of `data` is
///
/// ```text
/// append_chunk_with_length(file, 1000, BINARY_CHUNK, &[1, 2])  ->  file gains 1000, BIN, 1, 2
/// ```
pub(crate) fn append_chunk_with_length(file: &mut Vec<u8>, length: u32, chunk_type: u32, data: &[u8]) {
    file.extend_from_slice(&length.to_le_bytes());
    file.extend_from_slice(&chunk_type.to_le_bytes());
    file.extend_from_slice(data);
}

/// Appends a chunk whose header states the true length of `data`
///
/// ```text
/// append_chunk(file, BINARY_CHUNK, &[1, 2])  ->  file gains 2, BIN, 1, 2
/// ```
pub(crate) fn append_chunk(file: &mut Vec<u8>, chunk_type: u32, data: &[u8]) {
    let length = u32::try_from(data.len()).expect("test chunk fits in a u32");
    append_chunk_with_length(file, length, chunk_type, data);
}

/// Writes the true length of `file` into the total length field of its header
///
/// ```text
/// a 40-byte file whose header states 0  ->  the same file, its header stating 40
/// ```
pub(crate) fn write_total_length(file: &mut [u8]) {
    let length = u32::try_from(file.len()).expect("test file fits in a u32");
    file[8..HEADER_LENGTH].copy_from_slice(&length.to_le_bytes());
}

/// Pads `bytes` with `fill` until its length is a multiple of 4, as the glTF specification asks
/// of each chunk
///
/// ```text
/// bytes = [1, 2, 3, 4, 5], fill = 0  ->  [1, 2, 3, 4, 5, 0, 0, 0]
/// ```
pub(crate) fn pad_to_four(bytes: &mut Vec<u8>, fill: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(fill);
    }
}

/// Returns a GLB file with a JSON chunk that contains `json`, and a BIN chunk that contains
/// `binary`, and a header that states the length of the whole file
///
/// Each chunk contains its bytes exactly as given. The specification pads every chunk to a
/// multiple of four bytes, and a caller that wants a padded chunk pads it first with
/// `pad_to_four`. `split_glb` does not check the padding, and the tests that pass a chunk of two
/// bytes rely on that.
///
/// ```text
/// build_glb(b"{}", &[1, 2, 3, 4])  ->  a 34-byte file: header, JSON chunk, BIN chunk
/// ```
pub(crate) fn build_glb(json: &[u8], binary: &[u8]) -> Vec<u8> {
    let mut file = build_header(MAGIC, VERSION, 0);
    append_chunk(&mut file, JSON_CHUNK, json);
    append_chunk(&mut file, BINARY_CHUNK, binary);
    write_total_length(&mut file);
    file
}
