use crate::GltfError;

/// Reads a byte array from a large array, using the given `offset`
pub(crate) fn read_bytes_from<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], GltfError> {
    let end = offset + N;
    let slice = bytes.get(offset..end).ok_or(GltfError::Truncated)?;
    let mut arr = [0; N];
    arr.copy_from_slice(slice); // Copies the N bytes into a fixed-size array
    Ok(arr)
}