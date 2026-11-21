//! Holds the binary glTF files that the tests read, each one a fixture in `crates/hermes/fixtures`.
//!
//! The well-formed files come first. The files under `fixtures/broken` follow, and each of them
//! breaks one rule that `read_mesh` checks.
//!
//! The fixtures belong to the tests rather than to the game. A change to the game's own assets
//! therefore never reaches them.

/// Contains a copy of the hand model, frozen on 2026-11-21 from the export in `assets`.
///
/// The hand gives the tests a large file that Blender wrote, with a skin and poses. Later exports of
/// the game's hand leave this copy unchanged.
pub(crate) const HAND_MODEL: &[u8] = include_bytes!("../fixtures/hand.glb");

/// Contains a single triangle, with values chosen so that each one appears in one place only.
///
/// Every position has three different coordinates, and each vertex has a different normal. A
/// reader that swaps coordinates or vertices therefore returns different values. The indices are
/// stored as `u16`.
///
/// | Vertex | Position          | Normal            |
/// |--------|-------------------|-------------------|
/// | 0      | `[1.0, 2.0, 3.0]` | `[1.0, 0.0, 0.0]` |
/// | 1      | `[4.0, 6.0, 5.0]` | `[0.0, 1.0, 0.0]` |
/// | 2      | `[9.0, 7.0, 8.0]` | `[0.0, 0.0, 1.0]` |
///
/// The indices are `[0, 1, 2]`.
pub(crate) const TRIANGLE_MODEL: &[u8] = include_bytes!("../fixtures/triangle.glb");

/// Contains a cube with sides of length 2, centred on the origin, with flat faces.
///
/// Each of the six faces has four vertices of its own, which all take the face's outward normal.
/// The cube therefore has 24 vertices and 12 triangles, and its indices are stored as `u32`. Every
/// triangle winds counter-clockwise when seen from outside the cube.
///
/// The faces come in the order +X, -X, +Y, -Y, +Z, -Z. Face `f` takes vertices `4f` to `4f + 3`,
/// and its two triangles are `[4f, 4f + 1, 4f + 2]` and `[4f, 4f + 2, 4f + 3]`.
pub(crate) const CUBE_MODEL: &[u8] = include_bytes!("../fixtures/cube.glb");

/// Contains a file with a JSON text and no binary chunk, and with no mesh.
pub(crate) const WITHOUT_BINARY: &[u8] = include_bytes!("../fixtures/broken/without-binary.glb");

/// Contains a file with a binary chunk of four zero bytes, and with no mesh.
pub(crate) const WITHOUT_MESH: &[u8] = include_bytes!("../fixtures/broken/without-mesh.glb");

/// Contains the triangle of [`TRIANGLE_MODEL`] with its normals left out.
pub(crate) const WITHOUT_NORMALS: &[u8] =
    include_bytes!("../fixtures/broken/without-normals.glb");

/// Contains the triangle of [`TRIANGLE_MODEL`] with its indices left out.
pub(crate) const WITHOUT_INDICES: &[u8] =
    include_bytes!("../fixtures/broken/without-indices.glb");

/// Contains the triangle of [`TRIANGLE_MODEL`] with only the normals of its first two vertices, which
/// leaves 3 positions beside 2 normals.
pub(crate) const TOO_FEW_NORMALS: &[u8] =
    include_bytes!("../fixtures/broken/too-few-normals.glb");

/// Contains the triangle of [`TRIANGLE_MODEL`] with the indices `[0, 1, 3]`, where index 3 points
/// past the last of its 3 vertices.
pub(crate) const INDEX_PAST_LAST_VERTEX: &[u8] =
    include_bytes!("../fixtures/broken/index-past-last-vertex.glb");
