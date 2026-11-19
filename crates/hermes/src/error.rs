//! Defines the error that the crate returns when it cannot read a model.

use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

/// Describes why [`read_mesh`](crate::read_mesh) or [`read_model`](crate::read_model) could not
/// read a file.
///
/// Each variant names one part of the file that is missing or does not fit the rest. The
/// `# Errors` sections of the two functions list which part causes each variant.
#[derive(Debug)]
pub enum GltfError {
    // Wraps the gltf crate's own error: the file is not glTF, is malformed, or its JSON text
    // points at something that does not exist
    Gltf(gltf::Error),
    // The file has no binary chunk, which stores the numbers that the accessors point into
    MissingBinary,
    /// The file contains no mesh, or its first mesh has no primitive
    MissingMesh,
    // The first primitive has no `POSITION` attribute, or its positions cannot be read
    MissingPositions,
    /// The primitive that `read_mesh` and `read_model` read has no `indices`
    MissingIndices,
    /// The primitive that `read_mesh` and `read_model` read has no `NORMAL` attribute
    MissingNormals,
    /// The primitive that `read_mesh` and `read_model` read has a different number of normals from
    /// its number of positions, where glTF requires one normal for each position
    ///
    /// ```text
    /// 3 positions, 2 normals  ->  NormalCountMismatch { positions: 3, normals: 2 }
    /// ```
    NormalCountMismatch { positions: usize, normals: usize },
    /// An index of the primitive that `read_mesh` and `read_model` read points past its last vertex
    ///
    /// `index` is the first such index in the order that the file stores them.
    ///
    /// ```text
    /// 3 vertices, indices [0, 1, 3]  ->  IndexOutOfRange { index: 3, vertex_count: 3 }
    /// ```
    IndexOutOfRange { index: u32, vertex_count: usize },
    // The file has no skin, or the first primitive has no `JOINTS_0` or `WEIGHTS_0` that can be read
    MissingSkin,
    // A list in the skin has a different length from the list that it must match
    SkinCountMismatch { expected: usize, found: usize },
    // A vertex names a bone number past the last bone of the skin
    JointOutOfRange { joint: u16, bone_count: usize },
    // A bone comes before its parent in the skin's list of bones
    ParentAfterChild(usize),
    // An animation turns a node that is not a bone, or its rotations cannot be read
    InvalidPose(usize),
}

impl Display for GltfError {
    /// Writes a message of one line, starting in lower case, which states what is wrong with the
    /// file and any numbers that the variant contains.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Writes one line for each variant, starting in lower case
        match self {
            // States that the gltf crate rejected the file; `source` returns the details
            Self::Gltf(_) => write!(f, "the gltf crate could not read the file"),
            // States that the numbers are missing
            Self::MissingBinary => write!(f, "the file has no binary chunk"),
            Self::MissingMesh => {
                write!(f, "the file contains no mesh, or its first mesh has no primitive")
            }
            // States that the primitive has no positions
            Self::MissingPositions => write!(f, "the first primitive has no positions"),
            Self::MissingIndices => write!(f, "the first primitive has no indices"),
            Self::MissingNormals => write!(f, "the first primitive has no normals"),
            Self::NormalCountMismatch { positions, normals } => {
                write!(f, "the first primitive has {positions} positions, but {normals} normals")
            }
            Self::IndexOutOfRange { index, vertex_count } => {
                write!(f, "index {index} points past the last of {vertex_count} vertices")
            }
            // States that the skin, or the vertex data that ties the mesh to it, is missing
            Self::MissingSkin => {
                write!(f, "the file has no skin, or the first primitive has no joints or weights")
            }
            // States both lengths, so that the message shows how far apart they are
            Self::SkinCountMismatch { expected, found } => {
                write!(f, "the skin has {found} entries in a list that needs {expected}")
            }
            // States the bone number and how many bones exist
            Self::JointOutOfRange { joint, bone_count } => {
                write!(f, "joint {joint} points past the last of {bone_count} bones")
            }
            // States which bone comes too early
            Self::ParentAfterChild(bone) => write!(f, "bone {bone} comes before its parent"),
            // States which animation could not become a pose
            Self::InvalidPose(animation) => {
                write!(f, "animation {animation} turns a node that is not a bone, or has unreadable rotations")
            }
        }
    }
}

impl Error for GltfError {
    /// Returns the error from the `gltf` crate for [`GltfError::Gltf`], and `None` for every other
    /// variant.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            // Returns the gltf crate's own error, which states what it found wrong with the file
            Self::Gltf(error) => Some(error),
            // Returns `None`, since every other variant has no underlying error
            _ => None,
        }
    }
}

impl From<gltf::Error> for GltfError {
    /// Wraps `error` in [`GltfError::Gltf`].
    fn from(error: gltf::Error) -> Self {
        // Wraps the crate's error, which lets `?` turn a `gltf::Error` into a `GltfError`
        Self::Gltf(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Returns the error that the gltf crate raises for the unfinished JSON text `{`
    fn build_gltf_error() -> gltf::Error {
        // Parses `{`, which the crate rejects, and keeps the error
        gltf::Gltf::from_slice(b"{").err().unwrap()
    }

    /// Returns the name of the variant that `error` is
    ///
    /// The `match` has an arm for every variant, and has no wildcard arm. A new variant therefore stops the
    /// tests compiling until it gains an arm here, which is the prompt to add an instance of it to
    /// `build_every_variant` as well.
    fn name_variant(error: &GltfError) -> &'static str {
        match error {
            GltfError::Gltf(_) => "Gltf",
            GltfError::MissingBinary => "MissingBinary",
            GltfError::MissingMesh => "MissingMesh",
            GltfError::MissingPositions => "MissingPositions",
            GltfError::MissingIndices => "MissingIndices",
            GltfError::MissingNormals => "MissingNormals",
            GltfError::NormalCountMismatch { .. } => "NormalCountMismatch",
            GltfError::IndexOutOfRange { .. } => "IndexOutOfRange",
            GltfError::MissingSkin => "MissingSkin",
            GltfError::SkinCountMismatch { .. } => "SkinCountMismatch",
            GltfError::JointOutOfRange { .. } => "JointOutOfRange",
            GltfError::ParentAfterChild(_) => "ParentAfterChild",
            GltfError::InvalidPose(_) => "InvalidPose",
        }
    }

    /// Returns one instance of every variant, each beside the name that `name_variant` returns for it
    fn build_every_variant() -> Vec<(&'static str, GltfError)> {
        let variants = vec![
            GltfError::Gltf(build_gltf_error()),
            GltfError::MissingBinary,
            GltfError::MissingMesh,
            GltfError::MissingPositions,
            GltfError::MissingIndices,
            GltfError::MissingNormals,
            GltfError::NormalCountMismatch {
                positions: 3,
                normals: 2,
            },
            GltfError::IndexOutOfRange {
                index: 3,
                vertex_count: 3,
            },
            GltfError::MissingSkin,
            GltfError::SkinCountMismatch {
                expected: 22,
                found: 21,
            },
            GltfError::JointOutOfRange {
                joint: 22,
                bone_count: 22,
            },
            GltfError::ParentAfterChild(7),
            GltfError::InvalidPose(7),
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
        let cases: [(&str, GltfError, Vec<u128>); 4] = [
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

    /// Checks that `source` returns `None` for every variant other than `Gltf`
    #[test]
    fn source_with_every_other_variant_returns_none() {
        let variants: Vec<(&str, GltfError)> = build_every_variant()
            .into_iter()
            .filter(|(_, error)| !matches!(error, GltfError::Gltf(_)))
            .collect();

        let sources: Vec<(&str, bool)> = variants
            .iter()
            .map(|(name, error)| (*name, error.source().is_none()))
            .collect();

        for (name, is_none) in sources {
            assert!(is_none, "source for {name}");
        }
    }
}
