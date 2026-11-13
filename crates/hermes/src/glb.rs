//! Splits a GLB file into its JSON and binary chunks.
//!
//! The constants in this module come from
//! [Binary glTF Layout](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#binary-gltf-layout)
//! in the glTF 2.0 specification.

use crate::GltfError;
use crate::bytes::read_bytes_from;

const MAGIC: u32 = 0x4654_6C67;
const VERSION: u32 = 2;
const HEADER_LENGTH: usize = 12;
const CHUNK_HEADER_LENGTH: usize = 8;
const JSON_CHUNK: u32 = 0x4E4F_534A;
const BINARY_CHUNK: u32 = 0x004E_4942;

pub(crate) struct Glb<'a> {
    pub(crate) json: &'a [u8],
    pub(crate) binary: &'a [u8],
}

pub(crate) fn split_glb(file: &[u8]) -> Result<Glb<'_>, GltfError> {
    let magic = u32::from_le_bytes(read_bytes_from(file, 0)?);
    // Ensure the bytes being parsed are, in fact, GLTF bytes
    if magic != MAGIC {
        return Err(GltfError::NotGlb);
    }

    let version = u32::from_le_bytes(read_bytes_from(file, 4)?);
    if version != VERSION {
        return Err(GltfError::UnsupportedVersion(version));
    }

    let stated_length = u32::from_le_bytes(read_bytes_from(file, 8)?);
    let (json, json_end) = read_chunk(file, HEADER_LENGTH, JSON_CHUNK)?;
    let (binary, _) = read_chunk(file, json_end, BINARY_CHUNK)?;

    // Checks the length last, so that a file cut short inside a chunk reports `Truncated`
    let actual_length = file.len();
    let is_length_correct = usize::try_from(stated_length).is_ok_and(|stated| stated == actual_length);
    if !is_length_correct {
        return Err(GltfError::LengthMismatch { stated: stated_length, actual: actual_length });
    }

    Ok(Glb {json, binary})
}

fn read_chunk(file: &[u8], offset: usize, expected: u32) -> Result<(&[u8], usize), GltfError> {
    let length = u32::from_le_bytes(read_bytes_from(file, offset)?) as usize;
    let found = u32::from_le_bytes(read_bytes_from(file, offset + 4)?);
    if found != expected {
        return Err(GltfError::UnexpectedChunk {expected, found });
    }

    let data_start = offset + CHUNK_HEADER_LENGTH;
    let data_end = data_start + length;
    let data = file.get(data_start..data_end).ok_or(GltfError::Truncated)?;

    Ok((data, data_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Holds the bytes of the GLB file that Blender exported for the hand model
    const HAND_MODEL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/hand-model.glb"
    ));

    /// Stands for a chunk type that the glTF specification does not define
    const UNKNOWN_CHUNK: u32 = 0x5458_4554;

    /// Returns the 12-byte GLB header with the given `magic`, `version` and total `length`
    fn build_header(magic: u32, version: u32, length: u32) -> Vec<u8> {
        let mut header = Vec::with_capacity(HEADER_LENGTH);
        header.extend_from_slice(&magic.to_le_bytes());
        header.extend_from_slice(&version.to_le_bytes());
        header.extend_from_slice(&length.to_le_bytes());
        header
    }

    /// Appends a chunk whose header states `length`, whatever the true length of `data` is
    fn append_chunk_with_length(file: &mut Vec<u8>, length: u32, chunk_type: u32, data: &[u8]) {
        file.extend_from_slice(&length.to_le_bytes());
        file.extend_from_slice(&chunk_type.to_le_bytes());
        file.extend_from_slice(data);
    }

    /// Appends a chunk whose header states the true length of `data`
    fn append_chunk(file: &mut Vec<u8>, chunk_type: u32, data: &[u8]) {
        let length = u32::try_from(data.len()).expect("test chunk fits in a u32");
        append_chunk_with_length(file, length, chunk_type, data);
    }

    /// Writes the true length of `file` into the total length field of its header
    fn write_total_length(file: &mut [u8]) {
        let length = u32::try_from(file.len()).expect("test file fits in a u32");
        file[8..HEADER_LENGTH].copy_from_slice(&length.to_le_bytes());
    }

    /// Returns a GLB file with a JSON chunk holding `json` and a BIN chunk holding `binary`
    ///
    /// Each chunk holds its bytes exactly as given. The specification pads every chunk to a
    /// multiple of four bytes, so a caller that passes a length of 2 builds a file that a strict
    /// reader would reject. `split_glb` does not check the padding, and the tests rely on that.
    fn build_glb(json: &[u8], binary: &[u8]) -> Vec<u8> {
        let mut file = build_header(MAGIC, VERSION, 0);
        append_chunk(&mut file, JSON_CHUNK, json);
        append_chunk(&mut file, BINARY_CHUNK, binary);
        write_total_length(&mut file);
        file
    }

    /// Checks that the hand model splits into a JSON chunk of JSON text, and a BIN chunk of the
    /// length that the JSON states for its buffer
    #[test]
    fn split_glb_with_hand_model_returns_json_and_binary_chunks() {
        let file = HAND_MODEL;

        let glb = split_glb(file).expect("the hand model is a valid GLB file");
        let json: serde_json::Value =
            serde_json::from_slice(glb.json).expect("the JSON chunk holds JSON text");
        let buffer_length = json["buffers"][0]["byteLength"].as_u64();

        assert_eq!(glb.json.len(), 36_052, "length of the JSON chunk");
        assert_eq!(glb.json.first(), Some(&b'{'), "first byte of the JSON chunk");
        assert_eq!(glb.binary.len(), 227_716, "length of the BIN chunk");
        assert_eq!(buffer_length, Some(227_716), "byteLength of the first buffer");
    }

    /// Checks that the header and the two chunks of the hand model fill the file, with no byte
    /// left over, and that the header states the length of the file
    #[test]
    fn split_glb_with_hand_model_returns_chunks_that_fill_the_file() {
        let file = HAND_MODEL;

        let glb = split_glb(file).expect("the hand model is a valid GLB file");
        let chunk_lengths = glb.json.len() + glb.binary.len();
        let total_length = HEADER_LENGTH + 2 * CHUNK_HEADER_LENGTH + chunk_lengths;
        let stated_length = u32::from_le_bytes(read_bytes_from(file, 8).unwrap());

        assert_eq!(total_length, file.len(), "header and chunks against the file length");
        assert_eq!(stated_length, 263_796, "total length that the header states");
        assert_eq!(stated_length as usize, file.len(), "stated length against the file length");
    }

    /// Checks that `split_glb` returns the bytes of each chunk unchanged, including empty chunks
    #[test]
    fn split_glb_with_standard_and_extreme_chunks_returns_chunk_data() {
        let cases: [(&str, &[u8], &[u8]); 5] = [
            ("standard chunks", b"{\"asset\":{}}", &[1, 2, 3, 4, 5, 6, 7, 8]),
            ("single byte chunks", b"{", &[0xFF]),
            ("empty JSON chunk", b"", &[1, 2, 3, 4]),
            ("empty BIN chunk", b"{}  ", b""),
            ("both chunks empty", b"", b""),
        ];

        for (name, json, binary) in cases {
            let file = build_glb(json, binary);

            let result = split_glb(&file);

            assert!(
                matches!(result, Ok(Glb { json: found_json, binary: found_binary }) if found_json == json && found_binary == binary),
                "chunks for {name}"
            );
        }
    }

    /// Checks that a chunk after the BIN chunk is ignored, as the specification requires of a
    /// chunk type that a reader does not know
    #[test]
    fn split_glb_with_chunk_after_binary_chunk_returns_first_two_chunks() {
        let mut file = build_header(MAGIC, VERSION, 0);
        append_chunk(&mut file, JSON_CHUNK, b"{}  ");
        append_chunk(&mut file, BINARY_CHUNK, &[1, 2, 3, 4]);
        append_chunk(&mut file, UNKNOWN_CHUNK, &[9, 9, 9, 9]);
        write_total_length(&mut file);

        let result = split_glb(&file);

        assert!(
            matches!(result, Ok(Glb { json, binary }) if json == b"{}  " && binary == [1, 2, 3, 4]),
            "chunks of a file that ends with an unknown chunk"
        );
    }

    /// Checks that a file whose first four bytes are not `glTF` is rejected as not a GLB file
    #[test]
    fn split_glb_with_wrong_magic_returns_not_glb() {
        let cases = [
            ("zero magic", 0),
            ("magic in big-endian order", MAGIC.swap_bytes()),
            ("JSON chunk type as magic", JSON_CHUNK),
            ("magic one higher", MAGIC + 1),
            ("largest magic", u32::MAX),
        ];

        for (name, magic) in cases {
            let mut file = build_glb(b"{}", &[1, 2, 3, 4]);
            file[..4].copy_from_slice(&magic.to_le_bytes());

            let result = split_glb(&file);

            assert!(matches!(result, Err(GltfError::NotGlb)), "error for {name}");
        }
    }

    /// Checks that a header stating any version other than 2 is rejected, with that version
    #[test]
    fn split_glb_with_other_versions_returns_unsupported_version() {
        let versions = [0, 1, 3, u32::MAX];

        for version in versions {
            let mut file = build_glb(b"{}", &[1, 2, 3, 4]);
            file[4..8].copy_from_slice(&version.to_le_bytes());

            let result = split_glb(&file);

            assert!(
                matches!(result, Err(GltfError::UnsupportedVersion(found)) if found == version),
                "error for version {version}"
            );
        }
    }

    /// Checks that a file which ends inside the header, the JSON chunk header or the JSON chunk
    /// data is rejected as truncated
    #[test]
    fn split_glb_with_file_ending_before_json_chunk_ends_returns_truncated() {
        let valid = build_glb(b"{}  ", &[1, 2, 3, 4]);
        let json_start = HEADER_LENGTH + CHUNK_HEADER_LENGTH;
        let cases = [
            ("empty file", 0),
            ("part of the magic", 3),
            ("magic alone", 4),
            ("magic and part of the version", 7),
            ("magic and version", 8),
            ("header alone", HEADER_LENGTH),
            ("header and part of the JSON chunk header", HEADER_LENGTH + 4),
            ("header and JSON chunk header", json_start),
            ("first byte of the JSON chunk data", json_start + 1),
            ("all but the last byte of the JSON chunk data", json_start + 3),
        ];

        for (name, length) in cases {
            let file = &valid[..length];

            let result = split_glb(file);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that a header stating any total length but the length of the file is rejected,
    /// naming both lengths, from one byte off up to the largest length that a header can state
    #[test]
    fn split_glb_with_wrong_total_length_returns_length_mismatch() {
        let valid = build_glb(b"{}  ", &[1, 2, 3, 4]);
        let actual_length = valid.len();
        let true_length = u32::try_from(actual_length).expect("test file fits in a u32");
        let cases = [
            ("one byte more than the file", true_length + 1),
            ("one byte less than the file", true_length - 1),
            ("zero length", 0),
            ("largest length", u32::MAX),
        ];

        for (name, stated_length) in cases {
            let mut file = valid.clone();
            file[8..HEADER_LENGTH].copy_from_slice(&stated_length.to_le_bytes());

            let result = split_glb(&file);

            assert!(
                matches!(
                    result,
                    Err(GltfError::LengthMismatch { stated, actual })
                        if stated == stated_length && actual == actual_length
                ),
                "error for {name}"
            );
        }
    }

    /// Checks that a first chunk of any type but JSON is rejected, naming both chunk types
    #[test]
    fn split_glb_with_wrong_first_chunk_type_returns_unexpected_chunk() {
        let cases = [
            ("BIN chunk before JSON chunk", BINARY_CHUNK),
            ("unknown chunk type", UNKNOWN_CHUNK),
            ("zero chunk type", 0),
        ];

        for (name, first_type) in cases {
            let mut file = build_header(MAGIC, VERSION, 0);
            append_chunk(&mut file, first_type, &[1, 2, 3, 4]);
            append_chunk(&mut file, BINARY_CHUNK, &[1, 2, 3, 4]);
            write_total_length(&mut file);

            let result = split_glb(&file);

            assert!(
                matches!(
                    result,
                    Err(GltfError::UnexpectedChunk { expected: JSON_CHUNK, found }) if found == first_type
                ),
                "error for {name}"
            );
        }
    }

    /// Checks that a second chunk of any type but BIN is rejected, naming both chunk types
    #[test]
    fn split_glb_with_wrong_second_chunk_type_returns_unexpected_chunk() {
        let cases = [
            ("second JSON chunk", JSON_CHUNK),
            ("unknown chunk type", UNKNOWN_CHUNK),
            ("zero chunk type", 0),
        ];

        for (name, second_type) in cases {
            let mut file = build_header(MAGIC, VERSION, 0);
            append_chunk(&mut file, JSON_CHUNK, b"{}  ");
            append_chunk(&mut file, second_type, &[1, 2, 3, 4]);
            write_total_length(&mut file);

            let result = split_glb(&file);

            assert!(
                matches!(
                    result,
                    Err(GltfError::UnexpectedChunk { expected: BINARY_CHUNK, found }) if found == second_type
                ),
                "error for {name}"
            );
        }
    }

    /// Checks that a JSON chunk whose stated length runs past the end of the file is rejected,
    /// from one byte past the end up to the largest length that a chunk header can state
    #[test]
    fn split_glb_with_json_length_past_end_returns_truncated() {
        // The JSON data starts at byte 20 of a 32-byte file, so 12 bytes follow its start
        let lengths = [13, 1_000, u32::MAX];

        for length in lengths {
            let mut file = build_header(MAGIC, VERSION, 0);
            append_chunk_with_length(&mut file, length, JSON_CHUNK, b"{}  ");
            append_chunk(&mut file, BINARY_CHUNK, &[]);
            write_total_length(&mut file);

            let result = split_glb(&file);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "error for JSON chunk length {length}"
            );
        }
    }

    /// Checks that a BIN chunk whose stated length runs past the end of the file is rejected, up
    /// to the largest length that a chunk header can state
    #[test]
    fn split_glb_with_binary_length_past_end_returns_truncated() {
        let lengths = [5, 13, u32::MAX];

        for length in lengths {
            let mut file = build_header(MAGIC, VERSION, 0);
            append_chunk(&mut file, JSON_CHUNK, b"{}  ");
            append_chunk_with_length(&mut file, length, BINARY_CHUNK, &[1, 2, 3, 4]);
            write_total_length(&mut file);

            let result = split_glb(&file);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "error for BIN chunk length {length}"
            );
        }
    }

    /// Checks that a file which ends before the header of its BIN chunk is complete is rejected
    #[test]
    fn split_glb_with_missing_binary_chunk_returns_truncated() {
        let valid = build_glb(b"{}  ", &[1, 2, 3, 4]);
        let json_end = HEADER_LENGTH + CHUNK_HEADER_LENGTH + 4;
        let cases = [
            ("file ending after the JSON chunk", json_end),
            ("part of the BIN chunk length", json_end + 2),
            ("BIN chunk length alone", json_end + 4),
            ("part of the BIN chunk type", json_end + 6),
        ];

        for (name, length) in cases {
            let file = &valid[..length];

            let result = split_glb(file);

            assert!(matches!(result, Err(GltfError::Truncated)), "error for {name}");
        }
    }

    /// Checks that `read_chunk` returns the data of a chunk and the offset just past its end,
    /// including for an empty chunk and a chunk that ends exactly at the end of the file
    #[test]
    fn read_chunk_with_standard_and_extreme_chunks_returns_data_and_end() {
        let mut file = vec![0xAA; 3];
        append_chunk(&mut file, BINARY_CHUNK, &[1, 2, 3, 4]);
        append_chunk(&mut file, BINARY_CHUNK, &[]);
        append_chunk(&mut file, BINARY_CHUNK, &[5, 6]);
        let cases: [(&str, usize, &[u8], usize); 3] = [
            ("chunk at an unaligned offset", 3, &[1, 2, 3, 4], 15),
            ("empty chunk", 15, &[], 23),
            ("chunk ending at the end of the file", 23, &[5, 6], 33),
        ];

        for (name, offset, expected_data, expected_end) in cases {
            let result = read_chunk(&file, offset, BINARY_CHUNK);

            assert!(
                matches!(result, Ok((data, end)) if data == expected_data && end == expected_end),
                "data and end for {name}"
            );
        }
    }

    /// Checks that `read_chunk` rejects an offset that leaves no room for a chunk header
    #[test]
    fn read_chunk_with_offset_past_end_returns_truncated() {
        let file = build_glb(b"{}", &[1, 2, 3, 4]);
        let offsets = [file.len() - 4, file.len(), file.len() + 1, file.len() + 100];

        for offset in offsets {
            let result = read_chunk(&file, offset, BINARY_CHUNK);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "error for offset {offset} in a file of {} bytes",
                file.len()
            );
        }
    }

    /// Checks that `read_chunk` rejects a chunk of any other type, naming both chunk types
    #[test]
    fn read_chunk_with_other_chunk_type_returns_unexpected_chunk() {
        let cases = [
            ("JSON chunk where BIN is expected", JSON_CHUNK, BINARY_CHUNK),
            ("BIN chunk where JSON is expected", BINARY_CHUNK, JSON_CHUNK),
            ("unknown chunk where BIN is expected", UNKNOWN_CHUNK, BINARY_CHUNK),
        ];

        for (name, found_type, expected_type) in cases {
            let mut file = vec![0xAA; 3];
            append_chunk(&mut file, found_type, &[1, 2, 3, 4]);

            let result = read_chunk(&file, 3, expected_type);

            assert!(
                matches!(
                    result,
                    Err(GltfError::UnexpectedChunk { expected, found })
                        if expected == expected_type && found == found_type
                ),
                "error for {name}"
            );
        }
    }

    /// Checks that `read_chunk` rejects a chunk whose data runs past the end of the file, from one
    /// byte past it up to the largest length that a chunk header can state
    #[test]
    fn read_chunk_with_data_past_end_returns_truncated() {
        let lengths = [5, 1_000, u32::MAX];

        for length in lengths {
            let mut file = vec![0xAA; 3];
            append_chunk_with_length(&mut file, length, BINARY_CHUNK, &[1, 2, 3, 4]);

            let result = read_chunk(&file, 3, BINARY_CHUNK);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "error for chunk length {length}"
            );
        }
    }
}