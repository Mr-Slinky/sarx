use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum GltfError {
    // The file does not start with the four bytes `glTF`
    NotGlb,
    // The header states a glTF version other than 2
    UnsupportedVersion(u32),
    // A read ran past the end of the file or of its binary chunk
    Truncated,
    // A chunk has a different type from the one that its place in the file calls for
    UnexpectedChunk { expected: u32, found: u32 },
    // The JSON text does not match the structs that the reader declares
    Json(serde_json::Error),
    // The file contains no mesh, or its first mesh has no primitive
    MissingMesh,
    // An index points past the end of the accessor list
    MissingAccessor(usize),
    // An index points past the end of the buffer view list
    MissingBufferView(usize),
    // An accessor stores a number type or a shape that the reader does not handle
    UnsupportedAccessor(usize),
}

impl Display for GltfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Writes one line for each variant, starting in lower case
        match self {
            // States that the file is some other format
            Self::NotGlb => write!(f, "the file is not a binary glTF file"),
            // States the version that the file claims
            Self::UnsupportedVersion(version) => {
                write!(f, "the file uses glTF version {version}, and the reader handles version 2")
            }
            // States that the file ends too soon
            Self::Truncated => write!(f, "the file ends before the data that it describes"),
            // States both chunk types, in hexadecimal, as the glTF specification lists them
            Self::UnexpectedChunk { expected, found } => {
                write!(f, "expected a chunk of type {expected:#010x}, found {found:#010x}")
            }
            // States that the JSON text is the problem; `source` returns the details
            Self::Json(_) => write!(f, "the JSON text does not describe a mesh that the reader handles"),
            // States that the file has nothing to draw
            Self::MissingMesh => write!(f, "the file contains no mesh"),
            // States the index that points nowhere
            Self::MissingAccessor(index) => write!(f, "accessor {index} does not exist"),
            // States the index that points nowhere
            Self::MissingBufferView(index) => write!(f, "buffer view {index} does not exist"),
            // States which accessor the reader cannot handle
            Self::UnsupportedAccessor(index) => {
                write!(f, "accessor {index} stores a type that the reader does not handle")
            }
        }
    }
}

impl Error for GltfError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            // Hands back the serde_json error, which states the line and column of the problem
            Self::Json(error) => Some(error),
            // Returns `None`, since every other variant starts in the reader itself
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