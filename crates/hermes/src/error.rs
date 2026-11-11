use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum GltfError {
    /// The file does not start with the four bytes `glTF`
    NotGlb,
    /// The header states a glTF version other than 2
    UnsupportedVersion(u32),
    /// A read ran past the end of the file or of its binary chunk
    Truncated,
    /// A chunk has a different type from the one that its place in the file calls for
    UnexpectedChunk { expected: u32, found: u32 },
    /// The JSON text is malformed, or its fields do not match the types that it is parsed into
    Json(serde_json::Error),
    /// The file contains no mesh, or its first mesh has no primitive
    MissingMesh,
    /// An index points past the end of the accessor list
    MissingAccessor(usize),
    /// An index points past the end of the buffer view list
    MissingBufferView(usize),
    /// An accessor has an unsupported `componentType`, which defines the number type, or an
    /// unsupported `type`, which defines whether each element is a scalar, vector or matrix
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
            Self::Truncated => {
                write!(f, "the file or its binary chunk ends before the data that it describes")
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
            Self::MissingAccessor(index) => write!(f, "accessor {index} does not exist"),
            Self::MissingBufferView(index) => write!(f, "buffer view {index} does not exist"),
            Self::UnsupportedAccessor(index) => {
                write!(f, "accessor {index} has an unsupported component type or element type")
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