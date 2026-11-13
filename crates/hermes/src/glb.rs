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

    let (json, json_end) = read_chunk(file, HEADER_LENGTH, JSON_CHUNK)?;
    let (binary, _) = read_chunk(file, json_end, BINARY_CHUNK)?;

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