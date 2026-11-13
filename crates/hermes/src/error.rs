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
    /// An accessor has an unsupported `componentType`, which defines the number type, or an
    /// unsupported `type`, which defines whether each element is a scalar, vector or matrix, or
    /// has no `bufferView`, which glTF uses for an accessor whose elements are all zero
    UnsupportedAccessor(usize),
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
        // Wraps the serde_json error in the `Json` variant
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

    /// Returns the name of the variant that `error` holds
    ///
    /// The `match` names every variant and has no wildcard arm. A new variant therefore stops the
    /// tests compiling until it gains an arm here, which is the prompt to add an instance of it to
    /// `build_every_variant` as well.
    fn name_variant(error: &GltfError) -> &'static str {
        match error {
            GltfError::NotGlb => "NotGlb",
            GltfError::UnsupportedVersion(_) => "UnsupportedVersion",
            GltfError::Truncated => "Truncated",
            GltfError::UnexpectedChunk { .. } => "UnexpectedChunk",
            GltfError::Json(_) => "Json",
            GltfError::MissingMesh => "MissingMesh",
            GltfError::MissingAccessor(_) => "MissingAccessor",
            GltfError::MissingBufferView(_) => "MissingBufferView",
            GltfError::UnsupportedAccessor(_) => "UnsupportedAccessor",
        }
    }

    /// Returns one instance of every variant, each beside the name that `name_variant` gives it
    fn build_every_variant() -> Vec<(&'static str, GltfError)> {
        let variants = vec![
            GltfError::NotGlb,
            GltfError::UnsupportedVersion(3),
            GltfError::Truncated,
            GltfError::UnexpectedChunk {
                expected: 0x4E4F_534A,
                found: 0x004E_4942,
            },
            GltfError::Json(build_json_error()),
            GltfError::MissingMesh,
            GltfError::MissingAccessor(7),
            GltfError::MissingBufferView(7),
            GltfError::UnsupportedAccessor(7),
        ];

        variants
            .into_iter()
            .map(|error| (name_variant(&error), error))
            .collect()
    }

    /// Returns each run of decimal digits in `message`, as a whole number in text form
    ///
    /// For example, `"accessor 70 does not exist"` gives `["70"]`. Matching a whole run, rather
    /// than any substring, stops the value `0` from matching the last digit of `70`.
    fn split_numbers(message: &str) -> Vec<&str> {
        message
            .split(|character: char| !character.is_ascii_digit())
            .filter(|run| !run.is_empty())
            .collect()
    }

    /// Returns the result of parsing `text` as JSON, with `?` converting any serde_json error
    fn parse_with_question_mark(text: &str) -> Result<serde_json::Value, GltfError> {
        let value = serde_json::from_str::<serde_json::Value>(text)?;
        Ok(value)
    }

    /// Returns a `Json` error through `?`, which boxes it as a `Box<dyn Error>`
    fn return_boxed_json_error() -> Result<(), Box<dyn Error>> {
        Err(GltfError::Json(build_json_error()))?;
        Ok(())
    }

    /// Checks that the instances that the tests share hold no variant twice
    #[test]
    fn build_every_variant_with_no_data_returns_each_variant_once() {
        let variants = build_every_variant();

        let mut names: Vec<&str> = variants.iter().map(|(name, _)| *name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), count, "distinct variants in {names:?}");
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

    /// Checks that every variant starts its message with a lower-case letter
    #[test]
    fn fmt_with_every_variant_starts_in_lower_case() {
        let variants = build_every_variant();

        let first_characters: Vec<(&str, Option<char>)> = variants
            .iter()
            .map(|(name, error)| (*name, error.to_string().chars().next()))
            .collect();

        for (name, first) in first_characters {
            assert!(
                first.is_some_and(char::is_lowercase),
                "first character of message for {name}: {first:?}"
            );
        }
    }

    /// Checks that the variants carrying an index or version include that value, as a whole number
    #[test]
    fn fmt_with_standard_and_extreme_values_includes_value() {
        let cases = [
            ("UnsupportedVersion 0", GltfError::UnsupportedVersion(0), "0".to_string()),
            ("UnsupportedVersion 3", GltfError::UnsupportedVersion(3), "3".to_string()),
            (
                "UnsupportedVersion u32::MAX",
                GltfError::UnsupportedVersion(u32::MAX),
                u32::MAX.to_string(),
            ),
            ("MissingAccessor 0", GltfError::MissingAccessor(0), "0".to_string()),
            ("MissingAccessor 7", GltfError::MissingAccessor(7), "7".to_string()),
            (
                "MissingAccessor usize::MAX",
                GltfError::MissingAccessor(usize::MAX),
                usize::MAX.to_string(),
            ),
            ("MissingBufferView 0", GltfError::MissingBufferView(0), "0".to_string()),
            ("MissingBufferView 7", GltfError::MissingBufferView(7), "7".to_string()),
            (
                "MissingBufferView usize::MAX",
                GltfError::MissingBufferView(usize::MAX),
                usize::MAX.to_string(),
            ),
            ("UnsupportedAccessor 0", GltfError::UnsupportedAccessor(0), "0".to_string()),
            ("UnsupportedAccessor 7", GltfError::UnsupportedAccessor(7), "7".to_string()),
            (
                "UnsupportedAccessor usize::MAX",
                GltfError::UnsupportedAccessor(usize::MAX),
                usize::MAX.to_string(),
            ),
        ];

        for (name, error, value) in cases {
            let message = error.to_string();
            let numbers = split_numbers(&message);

            assert!(
                numbers.contains(&value.as_str()),
                "value {value} in message for {name}: {message}"
            );
        }
    }

    /// Checks that an unexpected chunk states both chunk types in the eight-digit hexadecimal form
    /// that the glTF specification uses, whatever the case of the letters
    #[test]
    fn fmt_with_unexpected_chunk_includes_both_types_in_hexadecimal() {
        let cases = [
            ("JSON expected, BIN found", 0x4E4F_534A, 0x004E_4942, "0x4e4f534a", "0x004e4942"),
            ("BIN expected, JSON found", 0x004E_4942, 0x4E4F_534A, "0x004e4942", "0x4e4f534a"),
            ("zero and largest", 0, u32::MAX, "0x00000000", "0xffffffff"),
        ];

        for (name, expected, found, expected_hex, found_hex) in cases {
            let error = GltfError::UnexpectedChunk { expected, found };

            let message = error.to_string().to_lowercase();

            assert!(
                message.contains(expected_hex),
                "expected type in message for {name}: {message}"
            );
            assert!(
                message.contains(found_hex),
                "found type in message for {name}: {message}"
            );
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

    /// Checks that `?` passes a successful parse through unchanged
    #[test]
    fn from_with_question_mark_on_valid_json_returns_value() {
        let text = r#"{"asset":{"version":"2.0"}}"#;

        let result = parse_with_question_mark(text);

        assert!(
            result.is_ok_and(|value| value["asset"]["version"] == "2.0"),
            "parsed value"
        );
    }

    /// Checks that `?` boxes a `GltfError` as a `Box<dyn Error>` that keeps its message and source
    #[test]
    fn from_with_question_mark_into_boxed_error_keeps_message_and_source() {
        let message = GltfError::Json(build_json_error()).to_string();

        let result = return_boxed_json_error();
        let boxed = result.err();

        assert_eq!(
            boxed.as_ref().map(ToString::to_string),
            Some(message),
            "message of the boxed error"
        );
        assert!(
            boxed.as_ref().and_then(|error| error.source()).is_some(),
            "source of the boxed error"
        );
    }
}