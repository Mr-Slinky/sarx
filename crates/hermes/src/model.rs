use crate::GltfError;
use gltf::buffer::Source;
use gltf::{Buffer, Gltf, Primitive};
use crate::pose::{read_poses, PoseData};
use crate::skin::{read_skin, SkinData};

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

pub struct Model {
    pub mesh: MeshData,
    pub skin: SkinData,
    pub poses: Vec<PoseData>, // every one-keyframe animation in the file
}

pub fn read_mesh(file: &[u8]) -> Result<MeshData, GltfError> {
    let gltf = Gltf::from_slice(file)?;
    let binary = find_binary(&gltf)?;
    let primitive = find_primitive(&gltf)?;
    read_mesh_from(&primitive, binary)
}

pub fn read_model(file: &[u8]) -> Result<Model, GltfError> {
    let gltf = Gltf::from_slice(file)?;
    let binary = find_binary(&gltf)?;
    let primitive = find_primitive(&gltf)?;
    let mesh = read_mesh_from(&primitive, binary)?;
    let skin = read_skin(&gltf, &primitive, binary, mesh.positions.len())?;
    let poses = read_poses(&gltf, binary)?;
    Ok(Model { mesh, skin, poses })
}

pub(crate) fn find_buffer<'a>(buffer: Buffer<'_>, binary: &'a [u8]) -> Option<&'a [u8]> {
    match buffer.source() {
        // Returns the binary chunk for the buffer that lives inside the .glb file
        Source::Bin => Some(binary),
        // Returns `None` for a buffer in a separate file, which Sarx does not load
        Source::Uri(_) => None,
    }
}

fn find_binary(gltf: &Gltf) -> Result<&[u8], GltfError> {
    // Returns the binary chunk, or `MissingBinary` if the file has none
    gltf.blob.as_deref().ok_or(GltfError::MissingBinary)
}

fn find_primitive(gltf: &Gltf) -> Result<Primitive<'_>, GltfError> {
    // Takes the first mesh in the file
    let mesh = gltf.meshes().next().ok_or(GltfError::MissingMesh)?;
    // Returns the first primitive of that mesh
    mesh.primitives().next().ok_or(GltfError::MissingMesh)
}

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

// ========================================================================================== \\
//                                           Tests                                            \\
// ========================================================================================== \\

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::HAND_MODEL;

    /// Checks that the hand model gives one normal per position, and whole triangles of indices
    #[test]
    fn read_mesh_with_hand_model_returns_expected_counts() {
        let mesh = read_mesh(HAND_MODEL).unwrap();

        assert_eq!(mesh.positions.len(), 3558, "number of positions");
        assert_eq!(mesh.normals.len(), 3558, "number of normals");
        assert_eq!(mesh.indices.len(), 19788, "number of indices");
        assert_eq!(
            mesh.indices.len() % 3,
            0,
            "indices left over after the last whole triangle"
        );
    }

    /// Checks that the indices of the hand model reach its last vertex, and never point past it
    #[test]
    fn read_mesh_with_hand_model_returns_indices_within_vertices() {
        let mesh = read_mesh(HAND_MODEL).unwrap();

        let vertex_count = mesh.positions.len();
        let out_of_range = mesh
            .indices
            .iter()
            .filter(|&&index| index as usize >= vertex_count)
            .count();
        let largest = mesh.indices.iter().max();

        assert_eq!(
            out_of_range, 0,
            "indices past the last of {vertex_count} vertices"
        );
        assert_eq!(largest, Some(&3557), "largest index");
    }

    /// Checks that every normal of the hand model has a length of 1, within a small tolerance
    #[test]
    fn read_mesh_with_hand_model_returns_unit_normals() {
        let tolerance = 1e-5;

        let mesh = read_mesh(HAND_MODEL).unwrap();

        let not_unit = mesh
            .normals
            .iter()
            .filter(|[x, y, z]| ((x * x + y * y + z * z).sqrt() - 1.0).abs() > tolerance)
            .count();

        assert_eq!(not_unit, 0, "normals whose length is not 1");
    }

    /// Checks that the hand model gives the first position, normal and triangle that the file stores
    #[test]
    fn read_mesh_with_hand_model_returns_first_vertex_and_triangle() {
        let mesh = read_mesh(HAND_MODEL).unwrap();

        assert_eq!(
            mesh.positions[0],
            [-0.76153266, 0.15828243, -3.4236743],
            "first position"
        );
        assert_eq!(
            mesh.normals[0],
            [-0.882713, -0.46630687, -0.058100857],
            "first normal"
        );
        assert_eq!(mesh.indices[..3], [0, 960, 2738], "first triangle");
    }
}
