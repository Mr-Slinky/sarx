//! Reads the mesh of a model, and the whole model with its skin and poses.

use crate::GltfError;
use gltf::buffer::Source;
use gltf::{Buffer, Gltf, Primitive};
use crate::pose::{read_poses, PoseData};
use crate::skin::{read_skin, SkinData};

/// Stores the shape of a model, as a list of vertices and a list of triangles.
///
/// Vertex `i` has the position `positions[i]` and the normal `normals[i]`, and the two lists have
/// the same length. The [crate documentation](crate#mesh) explains normals and indices, and the
/// example on [`read_model`] reads a mesh.
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    /// Stores the direction that the surface faces at each vertex.
    pub normals: Vec<[f32; 3]>,
    /// Stores three vertex numbers for each triangle, each one below the number of vertices.
    pub indices: Vec<u32>,
}

/// Stores the mesh, the skin and the poses that [`read_model`] reads from one glTF file.
///
/// The skin has one entry in `joints` and `weights` for each vertex of the mesh, and each pose has
/// one rotation for each bone of the skin. The example on [`read_model`] reads a model.
pub struct Model {
    pub mesh: MeshData,
    pub skin: SkinData,
    pub poses: Vec<PoseData>, // every one-keyframe animation in the file
}

/// Reads the mesh from `file`, the bytes of a binary glTF file, and leaves out the skin and the
/// poses.
///
/// The mesh comes from the first primitive of the first mesh in the file. [`read_model`] reads the
/// same mesh, and its example shows what the mesh contains.
///
/// # Errors
///
/// Returns [`GltfError::Gltf`] if the `gltf` crate cannot parse `file`, and
/// [`GltfError::MissingBinary`] if the file has no block of bytes. Returns
/// [`GltfError::MissingMesh`] if the file has no primitive to read.
///
/// The primitive itself causes the other errors. Each one names what is wrong with it:
/// [`MissingPositions`](GltfError::MissingPositions),
/// [`MissingNormals`](GltfError::MissingNormals),
/// [`NormalCountMismatch`](GltfError::NormalCountMismatch),
/// [`MissingIndices`](GltfError::MissingIndices) or
/// [`IndexOutOfRange`](GltfError::IndexOutOfRange).
pub fn read_mesh(file: &[u8]) -> Result<MeshData, GltfError> {
    let gltf = Gltf::from_slice(file)?;
    let binary = find_binary(&gltf)?;
    let primitive = find_primitive(&gltf)?;
    read_mesh_from(&primitive, binary)
}

/// Reads the mesh, the skin and the poses from `file`, the bytes of a binary glTF file.
///
/// The mesh comes from the first primitive of the first mesh in the file, as with [`read_mesh`].
/// The skin comes from the first skin in the file. Each pose comes from one animation, and the
/// poses keep the order that the file stores the animations in. The
/// [crate documentation](crate#poses) sets out which animations count as poses.
///
/// # Errors
///
/// Returns each error that [`read_mesh`] returns, for the same reasons. Also returns:
///
/// - [`GltfError::MissingSkin`] if the file has no skin, or the primitive has no bone numbers or
///   weights for its vertices.
/// - [`GltfError::SkinCountMismatch`] if the primitive has a different number of bone numbers or
///   weights from its number of vertices, or the skin has a different number of inverse bind
///   matrices from its number of bones.
/// - [`GltfError::JointOutOfRange`] if a vertex names a bone number past the last bone.
/// - [`GltfError::ParentAfterChild`] if a bone comes before its parent in the list of bones.
/// - [`GltfError::InvalidPose`] if an animation rotates a node that is not a bone, or if the
///   `gltf` crate cannot reach the rotations of an animation.
///
/// # Examples
///
/// The example reads the hand model. It then looks at the shape of the mesh, the bone that the
/// first vertex follows, and the rotations of the fist pose.
///
/// ```
/// use hermes::read_model;
///
/// // Reads the hand model, which Blender exported as a binary glTF file
/// let file = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/hand-model.glb"));
/// let model = read_model(file)?;
///
/// // Checks that the mesh has one normal for each vertex, and three indices for each triangle
/// let mesh = &model.mesh;
/// assert_eq!(mesh.positions.len(), 3558);
/// assert_eq!(mesh.normals.len(), mesh.positions.len());
/// assert_eq!(mesh.indices.len() % 3, 0);
///
/// // Reads the bones that the first vertex follows, which is the tip of the little finger alone
/// let skin = &model.skin;
/// let joints = skin.joints[0];
/// let weights = skin.weights[0];
/// let first_bone = &skin.bones[usize::from(joints[0])];
/// assert_eq!(first_bone.name, "pinky-bone-tip");
/// assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
///
/// // Checks that every bone comes after its parent, and that the wrist, the first bone, has no
/// // parent
/// assert_eq!(skin.bones.len(), 22);
/// for (index, bone) in skin.bones.iter().enumerate() {
///     assert!(bone.parent.is_none_or(|parent| parent < index));
/// }
/// assert_eq!(skin.bones[0].name, "wrist");
/// assert_eq!(skin.bones[0].parent, None);
///
/// // Finds the fist pose, and takes each bone's rotation in it, or the bone's rotation at rest
/// // where the pose leaves the bone out
/// let fist = model
///     .poses
///     .iter()
///     .find(|pose| pose.name == "fist")
///     .expect("the hand model should have a fist pose");
/// let rotations: Vec<[f32; 4]> = fist
///     .rotations
///     .iter()
///     .zip(&skin.bones)
///     .map(|(rotation, bone)| rotation.unwrap_or(bone.rotation))
///     .collect();
/// assert_eq!(rotations.len(), skin.bones.len());
/// # Ok::<(), hermes::GltfError>(())
/// ```
pub fn read_model(file: &[u8]) -> Result<Model, GltfError> {
    let gltf = Gltf::from_slice(file)?;
    let binary = find_binary(&gltf)?;
    let primitive = find_primitive(&gltf)?;
    let mesh = read_mesh_from(&primitive, binary)?;
    let skin = read_skin(&gltf, &primitive, binary, mesh.positions.len())?;
    let poses = read_poses(&gltf, binary)?;
    Ok(Model { mesh, skin, poses })
}

/// Returns the bytes that `buffer` stores, which is `binary` for a buffer inside the `.glb` file.
///
/// Returns `None` for a buffer that lies in a separate file. A reader from the `gltf` crate that
/// takes this function as its source then returns `None` for every list in that buffer.
pub(crate) fn find_buffer<'a>(buffer: Buffer<'_>, binary: &'a [u8]) -> Option<&'a [u8]> {
    match buffer.source() {
        // Returns the binary chunk for the buffer that lives inside the .glb file
        Source::Bin => Some(binary),
        // Returns `None` for a buffer in a separate file, which Sarx does not load
        Source::Uri(_) => None,
    }
}

/// Returns the block of bytes inside `gltf`, which stores the numbers that the accessors point
/// into.
///
/// # Errors
///
/// Returns [`GltfError::MissingBinary`] if the file has no block of bytes.
fn find_binary(gltf: &Gltf) -> Result<&[u8], GltfError> {
    // Returns the binary chunk, or `MissingBinary` if the file has none
    gltf.blob.as_deref().ok_or(GltfError::MissingBinary)
}

/// Returns the first primitive of the first mesh in `gltf`.
///
/// # Errors
///
/// Returns [`GltfError::MissingMesh`] if the file has no mesh, or its first mesh has no primitive.
fn find_primitive(gltf: &Gltf) -> Result<Primitive<'_>, GltfError> {
    // Takes the first mesh in the file
    let mesh = gltf.meshes().next().ok_or(GltfError::MissingMesh)?;
    // Returns the first primitive of that mesh
    mesh.primitives().next().ok_or(GltfError::MissingMesh)
}

/// Reads the positions, normals and indices of `primitive` from `binary`, the block of bytes of its
/// file.
///
/// # Errors
///
/// Returns [`GltfError::MissingPositions`], [`GltfError::MissingNormals`] or
/// [`GltfError::MissingIndices`] if the primitive lacks that list, or the `gltf` crate cannot reach
/// it. Returns
/// [`GltfError::NormalCountMismatch`] if the number of normals differs from the number of
/// positions, and [`GltfError::IndexOutOfRange`] if an index points past the last vertex.
fn read_mesh_from(primitive: &Primitive, binary: &[u8]) -> Result<MeshData, GltfError> {
    // Builds a reader, which follows each accessor of the primitive into the binary chunk
    let reader = primitive.reader(|buffer| find_buffer(buffer, binary));
    // Reads one position for each vertex
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or(GltfError::MissingPositions)?
        .collect();
    // Reads one normal for each vertex
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .ok_or(GltfError::MissingNormals)?
        .collect();
    // glTF requires every attribute of a primitive to hold one value for each vertex
    if normals.len() != positions.len() {
        // Rejects the file, stating both counts
        return Err(GltfError::NormalCountMismatch {
            positions: positions.len(),
            normals: normals.len(),
        });
    }

    // Reads the indices, widening each one to a u32 whatever size the file stores
    let indices: Vec<u32> = reader
        .read_indices()
        .ok_or(GltfError::MissingIndices)?
        .into_u32()
        .collect();
    // Counts the vertices, which every index must stay below
    let vertex_count = positions.len();
    // Finds the first index that points at or past the last vertex
    let out_of_range = indices
        .iter()
        .find(|&&index| !usize::try_from(index).is_ok_and(|index| index < vertex_count));
    if let Some(&index) = out_of_range {
        // Rejects the file, stating the index and the vertex count
        return Err(GltfError::IndexOutOfRange {
            index,
            vertex_count,
        });
    }

    // Returns the mesh
    Ok(MeshData {
        positions,
        normals,
        indices,
    })
}

// ========================================================================================== \
//                                           Tests                                            \
// ========================================================================================== \

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        CUBE_MODEL, HAND_MODEL, INDEX_PAST_LAST_VERTEX, TOO_FEW_NORMALS, TRIANGLE_MODEL,
        WITHOUT_BINARY, WITHOUT_INDICES, WITHOUT_MESH, WITHOUT_NORMALS,
    };

    /// Checks that the triangle gives back the positions, normals and `u16` indices it stores, in
    /// the order it stores them
    #[test]
    fn read_mesh_with_triangle_returns_stored_values() {
        let expected_positions = [[1.0, 2.0, 3.0], [4.0, 6.0, 5.0], [9.0, 7.0, 8.0]];
        let expected_normals = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let expected_indices = [0, 1, 2];

        let mesh = read_mesh(TRIANGLE_MODEL).unwrap();

        assert_eq!(mesh.positions, expected_positions, "positions");
        assert_eq!(mesh.normals, expected_normals, "normals");
        assert_eq!(mesh.indices, expected_indices, "indices");
    }

    /// Checks that the cube gives back the positions, normals and `u32` indices of its six faces,
    /// in the order it stores them
    #[test]
    fn read_mesh_with_cube_returns_stored_values() {
        let expected_positions = [
            // +X
            [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0],
            // -X
            [-1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0],
            // +Y
            [-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0],
            // -Y
            [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0],
            // +Z
            [-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0],
            // -Z
            [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, -1.0, -1.0],
        ];
        let face_normals = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        let expected_normals: Vec<[f32; 3]> = face_normals
            .iter()
            .flat_map(|&normal| [normal; 4])
            .collect();
        let expected_indices: Vec<u32> = (0..6)
            .flat_map(|face| {
                let first = 4 * face;
                [first, first + 1, first + 2, first, first + 2, first + 3]
            })
            .collect();

        let mesh = read_mesh(CUBE_MODEL).unwrap();

        assert_eq!(mesh.positions, expected_positions, "positions");
        assert_eq!(mesh.normals, expected_normals, "normals");
        assert_eq!(mesh.indices, expected_indices, "indices");
    }

    /// Checks that the frozen hand, a large file that Blender wrote, gives back every vertex and
    /// index it stores
    #[test]
    fn read_mesh_with_hand_model_returns_every_vertex_and_index() {
        let mesh = read_mesh(HAND_MODEL).unwrap();

        assert_eq!(mesh.positions.len(), 3558, "positions");
        assert_eq!(mesh.normals.len(), 3558, "normals");
        assert_eq!(mesh.indices.len(), 19788, "indices");
    }

    /// Checks that each broken file gives back the error for the one rule it breaks
    #[test]
    fn read_mesh_with_broken_files_returns_matching_error() {
        let cases: [(&str, &[u8], fn(&GltfError) -> bool); 6] = [
            ("without binary", WITHOUT_BINARY, |error| {
                matches!(error, GltfError::MissingBinary)
            }),
            ("without mesh", WITHOUT_MESH, |error| {
                matches!(error, GltfError::MissingMesh)
            }),
            ("without normals", WITHOUT_NORMALS, |error| {
                matches!(error, GltfError::MissingNormals)
            }),
            ("without indices", WITHOUT_INDICES, |error| {
                matches!(error, GltfError::MissingIndices)
            }),
            ("too few normals", TOO_FEW_NORMALS, |error| {
                matches!(
                    error,
                    GltfError::NormalCountMismatch {
                        positions: 3,
                        normals: 2
                    }
                )
            }),
            ("index past last vertex", INDEX_PAST_LAST_VERTEX, |error| {
                matches!(
                    error,
                    GltfError::IndexOutOfRange {
                        index: 3,
                        vertex_count: 3
                    }
                )
            }),
        ];

        let results: Vec<(&str, Option<GltfError>, fn(&GltfError) -> bool)> = cases
            .into_iter()
            .map(|(name, file, is_expected)| (name, read_mesh(file).err(), is_expected))
            .collect();

        for (name, error, is_expected) in results {
            assert!(error.as_ref().is_some_and(is_expected), "error for {name}: {error:?}");
        }
    }
}
