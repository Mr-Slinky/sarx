use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum GltfError {
    /// The file does not start with the four bytes `glTF`
    NotGlb,
    /// The header states a glTF version other than 2
    UnsupportedVersion(u32),
    /// The header states a total length that differs from the length of the file
    LengthMismatch { stated: u32, actual: usize },
    /// A read ran past the end of the file, of its binary chunk, or of the buffer view that the
    /// accessor belongs to
    Truncated,
    /// A chunk has a different type from the one that its place in the file calls for
    UnexpectedChunk { expected: u32, found: u32 },
    /// The JSON text is malformed, or its fields do not match the types that it is parsed into
    Json(serde_json::Error),
    /// The file contains no mesh, or its first mesh has no primitive
    MissingMesh,
    /// The primitive that `read_mesh` reads has no `indices`
    MissingIndices,
    /// The primitive that `read_mesh` reads has no `NORMAL` attribute
    MissingNormals,
    /// An index points past the end of the accessor list
    MissingAccessor(usize),
    /// An index points past the end of the buffer view list
    MissingBufferView(usize),
    /// A buffer view states a `byteStride` that glTF forbids: one outside 4 to 252, one that is
    /// not a multiple of 4, one smaller than an element, or any stride on a view of indices
    InvalidStride(usize),
    /// An accessor has a form that the reader does not support
    ///
    /// An accessor states its number type in `componentType`, and whether each element is a
    /// scalar, vector or matrix in `type`. Either one can take a value that the reader does not
    /// support. The reader also returns this error for an accessor without a `bufferView`, which
    /// glTF leaves out where every element is zero.
    UnsupportedAccessor(usize),
    /// The primitive that `read_mesh` reads has a different number of normals from its number of
    /// positions, where glTF requires one normal for each position
    ///
    /// ```text
    /// 3 positions, 2 normals  ->  NormalCountMismatch { positions: 3, normals: 2 }
    /// ```
    NormalCountMismatch { positions: usize, normals: usize },
    /// An index of the primitive that `read_mesh` reads points at or past the end of its vertices
    ///
    /// `index` is the first such index in the order that the file stores them.
    ///
    /// ```text
    /// 3 vertices, indices [0, 1, 3]  ->  IndexOutOfRange { index: 3, vertex_count: 3 }
    /// ```
    IndexOutOfRange { index: u32, vertex_count: usize },
}

impl Display for GltfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Writes one line for each variant, starting in lower case
        match self {
            Self::NotGlb => write!(f, "the file is not a binary glTF file"),
            Self::UnsupportedVersion(version) => {
                write!(f, "expected glTF version 2, found version {version}")
            }
            Self::LengthMismatch { stated, actual } => {
                write!(f, "the header states a length of {stated} bytes, but the file has {actual}")
            }
            Self::Truncated => {
                write!(f, "the file, its binary chunk or a buffer view ends before the data that it describes")
            }
            // States both chunk types, in hexadecimal, as the glTF specification lists them
            Self::UnexpectedChunk { expected, found } => {
                write!(f, "expected a chunk of type {expected:#010x}, found {found:#010x}")
            }
            // States that the JSON text is the problem; `source` returns the details
            Self::Json(_) => {
                write!(f, "the JSON text is malformed, or its fields have unexpected types")
            }
            Self::MissingMesh => {
                write!(f, "the file contains no mesh, or its first mesh has no primitive")
            }
            Self::MissingIndices => write!(f, "the first primitive has no indices"),
            Self::MissingNormals => write!(f, "the first primitive has no normals"),
            Self::MissingAccessor(index) => write!(f, "accessor {index} does not exist"),
            Self::MissingBufferView(index) => write!(f, "buffer view {index} does not exist"),
            Self::InvalidStride(index) => write!(f, "buffer view {index} has a stride that glTF forbids"),
            Self::UnsupportedAccessor(index) => {
                write!(f, "accessor {index} has an unsupported component type or element type, or no buffer view")
            }
            Self::NormalCountMismatch { positions, normals } => {
                write!(f, "the first primitive has {positions} positions, but {normals} normals")
            }
            Self::IndexOutOfRange { index, vertex_count } => {
                write!(f, "index {index} points past the last of {vertex_count} vertices")
            }
        }
    }
}

impl Error for GltfError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            // Returns the serde_json error, which states the line and column of the problem
            Self::Json(error) => Some(error),
            // Returns `None`, since every other variant has no underlying error
            _ => None,
        }
    }
}

impl From<serde_json::Error> for GltfError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the serde_json error that parsing the unfinished JSON text `{` raises
    fn build_json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").unwrap_err()
    }

    /// Returns the name of the variant that `error` is
    ///
    /// The `match` has an arm for every variant, and has no wildcard arm. A new variant therefore stops the
    /// tests compiling until it gains an arm here, which is the prompt to add an instance of it to
    /// `build_every_variant` as well.
    fn name_variant(error: &GltfError) -> &'static str {
        match error {
            GltfError::NotGlb => "NotGlb",
            GltfError::UnsupportedVersion(_) => "UnsupportedVersion",
            GltfError::LengthMismatch { .. } => "LengthMismatch",
            GltfError::Truncated => "Truncated",
            GltfError::UnexpectedChunk { .. } => "UnexpectedChunk",
            GltfError::Json(_) => "Json",
            GltfError::MissingMesh => "MissingMesh",
            GltfError::MissingIndices => "MissingIndices",
            GltfError::MissingNormals => "MissingNormals",
            GltfError::MissingAccessor(_) => "MissingAccessor",
            GltfError::MissingBufferView(_) => "MissingBufferView",
            GltfError::InvalidStride(_) => "InvalidStride",
            GltfError::UnsupportedAccessor(_) => "UnsupportedAccessor",
            GltfError::NormalCountMismatch { .. } => "NormalCountMismatch",
            GltfError::IndexOutOfRange { .. } => "IndexOutOfRange",
        }
    }

    /// Returns one instance of every variant, each beside the name that `name_variant` returns for it
    fn build_every_variant() -> Vec<(&'static str, GltfError)> {
        let variants = vec![
            GltfError::NotGlb,
            GltfError::UnsupportedVersion(3),
            GltfError::LengthMismatch {
                stated: 120,
                actual: 96,
            },
            GltfError::Truncated,
            GltfError::UnexpectedChunk {
                expected: 0x4E4F_534A,
                found: 0x004E_4942,
            },
            GltfError::Json(build_json_error()),
            GltfError::MissingMesh,
            GltfError::MissingIndices,
            GltfError::MissingNormals,
            GltfError::MissingAccessor(7),
            GltfError::MissingBufferView(7),
            GltfError::InvalidStride(7),
            GltfError::UnsupportedAccessor(7),
            GltfError::NormalCountMismatch {
                positions: 3,
                normals: 2,
            },
            GltfError::IndexOutOfRange {
                index: 3,
                vertex_count: 3,
            },
        ];

        variants
            .into_iter()
            .map(|error| (name_variant(&error), error))
            .collect()
    }

    /// Returns each run of decimal digits in `message`, as a whole number in text form
    ///
    /// For example, `"accessor 70 does not exist"` returns `["70"]`. Matching a whole run, rather
    /// than any substring, stops the value `0` from matching the last digit of `70`.
    fn split_numbers(message: &str) -> Vec<&str> {
        message
            .split(|character: char| !character.is_ascii_digit())
            .filter(|run| !run.is_empty())
            .collect()
    }

    /// Returns whether `message` states `value` as a whole word, either in decimal or in
    /// hexadecimal, with or without a `0x` prefix, leading zeros and upper-case letters
    ///
    /// ```text
    /// message = "accessor 70 does not exist",  value = 70           ->  true
    /// message = "accessor 70 does not exist",  value = 0            ->  false
    /// message = "found 0x004E4942",            value = 0x004E_4942  ->  true
    /// ```
    fn contains_value(message: &str, value: u128) -> bool {
        let decimal = value.to_string();
        let hexadecimal = format!("{value:x}");
        let is_decimal = split_numbers(message).contains(&decimal.as_str());
        let lower_case = message.to_lowercase();
        let is_hexadecimal = lower_case
            .split(|character: char| !character.is_ascii_alphanumeric())
            .map(|word| word.strip_prefix("0x").unwrap_or(word))
            .any(|digits| {
                let significant = digits.trim_start_matches('0');
                let is_zero = !digits.is_empty() && significant.is_empty();
                significant == hexadecimal || (is_zero && hexadecimal == "0")
            });
        is_decimal || is_hexadecimal
    }

    /// Returns the result of parsing `text` as JSON, with `?` converting any serde_json error
    fn parse_with_question_mark(text: &str) -> Result<serde_json::Value, GltfError> {
        let value = serde_json::from_str::<serde_json::Value>(text)?;
        Ok(value)
    }

    /// Checks that every variant writes a message that is not empty
    #[test]
    fn fmt_with_every_variant_writes_non_empty_message() {
        let variants = build_every_variant();

        let messages: Vec<(&str, String)> = variants
            .iter()
            .map(|(name, error)| (*name, error.to_string()))
            .collect();

        for (name, message) in messages {
            assert!(!message.is_empty(), "message for {name}");
        }
    }

    /// Checks that every variant writes its message on a single line
    #[test]
    fn fmt_with_every_variant_writes_single_line() {
        let variants = build_every_variant();

        let messages: Vec<(&str, String)> = variants
            .iter()
            .map(|(name, error)| (*name, error.to_string()))
            .collect();

        for (name, message) in messages {
            assert!(
                !message.contains('\n'),
                "line breaks in message for {name}: {message}"
            );
        }
    }

    /// Checks that every variant that contains a value includes that value in its message, in
    /// decimal or in hexadecimal, whatever the form around it
    #[test]
    fn fmt_with_standard_and_extreme_values_includes_values() {
        let largest_usize = usize::MAX as u128;
        let largest_u32 = u128::from(u32::MAX);
        let cases: [(&str, GltfError, Vec<u128>); 22] = [
            ("UnsupportedVersion 0", GltfError::UnsupportedVersion(0), vec![0]),
            ("UnsupportedVersion 3", GltfError::UnsupportedVersion(3), vec![3]),
            ("UnsupportedVersion u32::MAX", GltfError::UnsupportedVersion(u32::MAX), vec![largest_u32]),
            ("LengthMismatch 120 and 96", GltfError::LengthMismatch { stated: 120, actual: 96 }, vec![120, 96]),
            (
                "LengthMismatch u32::MAX and usize::MAX",
                GltfError::LengthMismatch { stated: u32::MAX, actual: usize::MAX },
                vec![largest_u32, largest_usize],
            ),
            (
                "UnexpectedChunk JSON and BIN",
                GltfError::UnexpectedChunk { expected: 0x4E4F_534A, found: 0x004E_4942 },
                vec![0x4E4F_534A, 0x004E_4942],
            ),
            (
                "UnexpectedChunk 1 and u32::MAX",
                GltfError::UnexpectedChunk { expected: 1, found: u32::MAX },
                vec![1, largest_u32],
            ),
            ("MissingAccessor 0", GltfError::MissingAccessor(0), vec![0]),
            ("MissingAccessor 7", GltfError::MissingAccessor(7), vec![7]),
            ("MissingAccessor usize::MAX", GltfError::MissingAccessor(usize::MAX), vec![largest_usize]),
            ("MissingBufferView 0", GltfError::MissingBufferView(0), vec![0]),
            ("MissingBufferView 7", GltfError::MissingBufferView(7), vec![7]),
            ("MissingBufferView usize::MAX", GltfError::MissingBufferView(usize::MAX), vec![largest_usize]),
            ("InvalidStride 7", GltfError::InvalidStride(7), vec![7]),
            ("InvalidStride usize::MAX", GltfError::InvalidStride(usize::MAX), vec![largest_usize]),
            ("UnsupportedAccessor 0", GltfError::UnsupportedAccessor(0), vec![0]),
            ("UnsupportedAccessor 7", GltfError::UnsupportedAccessor(7), vec![7]),
            ("UnsupportedAccessor usize::MAX", GltfError::UnsupportedAccessor(usize::MAX), vec![largest_usize]),
            (
                "NormalCountMismatch 3 and 2",
                GltfError::NormalCountMismatch { positions: 3, normals: 2 },
                vec![3, 2],
            ),
            (
                "NormalCountMismatch 0 and usize::MAX",
                GltfError::NormalCountMismatch { positions: 0, normals: usize::MAX },
                vec![0, largest_usize],
            ),
            ("IndexOutOfRange 3 of 3", GltfError::IndexOutOfRange { index: 3, vertex_count: 3 }, vec![3]),
            (
                "IndexOutOfRange u32::MAX of 0",
                GltfError::IndexOutOfRange { index: u32::MAX, vertex_count: 0 },
                vec![largest_u32, 0],
            ),
        ];

        for (name, error, values) in cases {
            let message = error.to_string();

            for value in values {
                assert!(
                    contains_value(&message, value),
                    "value {value} in message for {name}: {message}"
                );
            }
        }
    }

    /// Checks that `source` returns the wrapped serde_json error, with its line and column intact
    #[test]
    fn source_with_json_returns_wrapped_error() {
        let original = build_json_error();
        let line = original.line();
        let column = original.column();
        let error = GltfError::Json(original);

        let source = error.source();
        let wrapped = source.and_then(|source| source.downcast_ref::<serde_json::Error>());

        assert!(wrapped.is_some(), "source downcast to a serde_json error");
        assert_eq!(
            wrapped.map(serde_json::Error::line),
            Some(line),
            "line of the wrapped error"
        );
        assert_eq!(
            wrapped.map(serde_json::Error::column),
            Some(column),
            "column of the wrapped error"
        );
    }

    /// Checks that `source` returns `None` for every variant other than `Json`
    #[test]
    fn source_with_every_other_variant_returns_none() {
        let variants: Vec<(&str, GltfError)> = build_every_variant()
            .into_iter()
            .filter(|(_, error)| !matches!(error, GltfError::Json(_)))
            .collect();

        let sources: Vec<(&str, bool)> = variants
            .iter()
            .map(|(name, error)| (*name, error.source().is_none()))
            .collect();

        for (name, is_none) in sources {
            assert!(is_none, "source for {name}");
        }
    }

    /// Checks that `from` wraps a serde_json error in the `Json` variant, unchanged
    #[test]
    fn from_with_json_error_returns_json_variant() {
        let original = build_json_error();
        let line = original.line();
        let column = original.column();

        let error = GltfError::from(original);

        assert!(
            matches!(&error, GltfError::Json(inner) if inner.line() == line && inner.column() == column),
            "converted error: {error:?}"
        );
    }

    /// Checks that `?` converts a serde_json error into the `Json` variant
    #[test]
    fn from_with_question_mark_on_malformed_json_returns_json_variant() {
        let text = "{";

        let result = parse_with_question_mark(text);

        assert!(
            matches!(result, Err(GltfError::Json(_))),
            "result: {result:?}"
        );
    }
}
