use crate::GltfError;

/// Reads a byte array from a large array, using the given `offset`
pub(crate) fn read_bytes_from<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], GltfError> {
    // Fails rather than wrapping round where an offset from a corrupt file nears `usize::MAX`
    let end = offset.checked_add(N).ok_or(GltfError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(GltfError::Truncated)?;
    let mut arr = [0; N];
    arr.copy_from_slice(slice); // Copies the N bytes into a fixed-size array
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds eight bytes, each holding its own index plus 10, so that every offset reads
    /// different values
    fn build_eight_bytes() -> [u8; 8] {
        [10, 11, 12, 13, 14, 15, 16, 17]
    }

    /// Checks that four bytes read from the start, the middle and the very end match the source
    #[test]
    fn read_bytes_from_with_standard_and_extreme_offsets_returns_bytes() {
        let bytes = build_eight_bytes();
        let cases = [
            ("first byte", 0, [10, 11, 12, 13]),
            ("middle", 2, [12, 13, 14, 15]),
            ("odd offset", 3, [13, 14, 15, 16]),
            ("exact last four bytes", 4, [14, 15, 16, 17]),
        ];

        for (name, offset, expected) in cases {
            let result = read_bytes_from::<4>(&bytes, offset);

            assert_eq!(result.ok(), Some(expected), "bytes read for {name}");
        }
    }

    /// Checks that reading one byte returns the byte at the offset, up to the last byte
    #[test]
    fn read_bytes_from_with_one_byte_returns_byte_at_offset() {
        let bytes = build_eight_bytes();
        let cases = [("first byte", 0, [10]), ("middle byte", 4, [14]), ("last byte", 7, [17])];

        for (name, offset, expected) in cases {
            let result = read_bytes_from::<1>(&bytes, offset);

            assert_eq!(result.ok(), Some(expected), "byte read for {name}");
        }
    }

    /// Checks that reading as many bytes as the slice holds returns every byte
    #[test]
    fn read_bytes_from_with_whole_slice_returns_every_byte() {
        let bytes = build_eight_bytes();

        let result = read_bytes_from::<8>(&bytes, 0);

        assert_eq!(result.ok(), Some(bytes), "bytes read for the whole slice");
    }

    /// Checks that reading no bytes succeeds anywhere from the start up to the end of the slice
    #[test]
    fn read_bytes_from_with_zero_length_returns_empty_array() {
        let bytes = build_eight_bytes();
        let cases = [
            ("start of slice", &bytes[..], 0),
            ("end of slice", &bytes[..], 8),
            ("empty slice", &[][..], 0),
        ];

        for (name, slice, offset) in cases {
            let result = read_bytes_from::<0>(slice, offset);

            assert_eq!(result.ok(), Some([]), "bytes read for {name}");
        }
    }

    /// Checks that a read running past the end of the slice returns `Truncated`
    #[test]
    fn read_bytes_from_with_read_past_end_returns_truncated() {
        let bytes = build_eight_bytes();
        let cases = [
            ("one byte short", &bytes[..], 5),
            ("last byte only", &bytes[..], 7),
            ("offset at end", &bytes[..], 8),
            ("offset past end", &bytes[..], 9),
            ("empty slice", &[][..], 0),
        ];

        for (name, slice, offset) in cases {
            let result = read_bytes_from::<4>(slice, offset);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "result for {name}"
            );
        }
    }

    /// Checks that reading one byte more than the slice holds returns `Truncated`
    #[test]
    fn read_bytes_from_with_length_longer_than_slice_returns_truncated() {
        let bytes = build_eight_bytes();

        let result = read_bytes_from::<9>(&bytes, 0);

        assert!(
            matches!(result, Err(GltfError::Truncated)),
            "result for nine bytes from an eight-byte slice"
        );
    }

    /// Checks that reading no bytes from an offset past the end of the slice returns `Truncated`
    #[test]
    fn read_bytes_from_with_zero_length_past_end_returns_truncated() {
        let bytes = build_eight_bytes();
        let offsets = [9, usize::MAX];

        for offset in offsets {
            let result = read_bytes_from::<0>(&bytes, offset);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "result for zero bytes at offset {offset}"
            );
        }
    }

    /// Checks that an offset whose read ends exactly at `usize::MAX` returns `Truncated`
    #[test]
    fn read_bytes_from_with_read_ending_at_usize_max_returns_truncated() {
        let bytes = build_eight_bytes();

        let result = read_bytes_from::<4>(&bytes, usize::MAX - 4);

        assert!(
            matches!(result, Err(GltfError::Truncated)),
            "result for a read ending at usize::MAX"
        );
    }

    /// Checks that an offset so large that adding the length overflows returns `Truncated`
    #[test]
    fn read_bytes_from_with_offset_near_usize_max_returns_truncated() {
        let bytes = build_eight_bytes();
        let offsets = [usize::MAX - 3, usize::MAX - 1, usize::MAX];

        for offset in offsets {
            let result = read_bytes_from::<4>(&bytes, offset);

            assert!(
                matches!(result, Err(GltfError::Truncated)),
                "result for offset {offset}"
            );
        }
    }
}
