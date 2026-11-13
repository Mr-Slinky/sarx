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

/// Returns every vec3 element in the file, in the same order that the file stores them
pub(crate) fn read_vec3s(document: &Document, binary: &[u8], index: usize) -> Result<Vec<[f32; 3]>, GltfError> {
    // Finds the accessor, or fails where the index points past the end of the list
    let accessor = document.accessors.get(index).ok_or(GltfError::MissingAccessor(index))?;
    let is_vec3 = accessor.component_type == FLOAT_CODE && accessor.kind == "VEC3";
    if !is_vec3 {
        return Err(GltfError::UnsupportedAccessor(index));
    }

    let layout = find_layout(document, accessor, VEC3_SIZE)?;
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

    let layout = find_layout(document, accessor, element_size)?;
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
}

fn find_layout(document: &Document, accessor: &Accessor, element_size: usize) -> Result<Layout, GltfError> {
    let view_index = accessor.buffer_view;
    let view = document.buffer_views.get(view_index).ok_or(GltfError::MissingBufferView(view_index))?;
    let start = view.byte_offset + accessor.byte_offset;
    let stride = view.byte_stride.unwrap_or(element_size);
    Ok(Layout { start, stride })
}