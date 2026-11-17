use crate::GltfError;
use crate::accessor::{read_indices, read_vec3s};
use crate::document::GlbDocument;
use crate::glb::split_glb;

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

pub fn read_mesh(file: &[u8]) -> Result<MeshData, GltfError> {
    let glb = split_glb(file)?;
    let document: GlbDocument = serde_json::from_slice(glb.json)?;
    let mesh = document.meshes.first().ok_or(GltfError::MissingMesh)?;
    let primitive = mesh.primitives.first().ok_or(GltfError::MissingMesh)?;
    let positions = read_vec3s(&document, glb.binary, primitive.attributes.position)?;
    let normal_index = primitive
        .attributes
        .normal
        .ok_or(GltfError::MissingNormals)?;
    let normals = read_vec3s(&document, glb.binary, normal_index)?;
    // glTF requires every attribute of a primitive to hold one value for each vertex
    if normals.len() != positions.len() {
        return Err(GltfError::NormalCountMismatch {
            positions: positions.len(),
            normals: normals.len(),
        });
    }

    let indices_index = primitive.indices.ok_or(GltfError::MissingIndices)?;
    let indices = read_indices(&document, glb.binary, indices_index)?;
    let vertex_count = positions.len();
    let out_of_range = indices
        .iter()
        .find(|&&index| !usize::try_from(index).is_ok_and(|index| index < vertex_count));
    if let Some(&index) = out_of_range {
        return Err(GltfError::IndexOutOfRange {
            index,
            vertex_count,
        });
    }

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
    use crate::test_support::{HAND_MODEL, build_glb, pad_to_four};

    /// The positions of the triangle that the fixture file stores
    const TRIANGLE_POSITIONS: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

    /// The normals of the triangle that the fixture file stores, all facing along `z`
    const TRIANGLE_NORMALS: [[f32; 3]; 3] = [[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];

    /// The indices of the triangle that the fixture file stores
    const TRIANGLE_INDICES: [u16; 3] = [0, 1, 2];

    /// Two triangles of indices that the fixture file stores after the triangle, each with one
    /// index past the last of the three vertices: one equal to the vertex count, and the largest
    /// 16-bit index
    const OUT_OF_RANGE_INDICES: [u16; 6] = [0, 1, 3, 0, 1, u16::MAX];

    /// The meshes of a fixture file with one mesh, whose only primitive points to the accessors
    /// for the triangle
    const TRIANGLE_MESHES: &str =
        r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":2}]}]"#;

    /// The accessors of the fixture file. Accessors 0 to 2 read the triangle. Accessor 3 has the
    /// unsupported type `VEC2`, accessor 4 counts more vec3s than the binary chunk contains, and
    /// accessors 5 and 6 have a count of zero. Accessor 7 points to buffer view 9, which does not
    /// exist, and accessor 8 counts more indices than the binary chunk contains. Accessors 4 and 8 stay
    /// inside their buffer views, so that the binary chunk is the bound that they cross. Accessor 9
    /// reads the first two normals alone. Accessors 10 and 11 read the two triangles of
    /// `OUT_OF_RANGE_INDICES`
    const FIXTURE_ACCESSORS: &str = r#"[
        {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
        {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},
        {"bufferView":2,"componentType":5123,"count":3,"type":"SCALAR"},
        {"bufferView":0,"componentType":5126,"count":3,"type":"VEC2"},
        {"bufferView":3,"componentType":5126,"count":100,"type":"VEC3"},
        {"bufferView":0,"componentType":5126,"count":0,"type":"VEC3"},
        {"bufferView":2,"componentType":5123,"count":0,"type":"SCALAR"},
        {"bufferView":9,"componentType":5126,"count":3,"type":"VEC3"},
        {"bufferView":4,"componentType":5123,"count":100,"type":"SCALAR"},
        {"bufferView":1,"componentType":5126,"count":2,"type":"VEC3"},
        {"bufferView":5,"componentType":5123,"count":3,"type":"SCALAR"},
        {"bufferView":5,"byteOffset":6,"componentType":5123,"count":3,"type":"SCALAR"}
    ]"#;

    /// The buffer views of the fixture file: the positions, the normals, then the indices. Views 3
    /// and 4 start where the positions and the indices start, and run past the 92-byte binary
    /// chunk: view 3 has room for 100 vec3s, and view 4 for 100 indices. View 5 holds
    /// `OUT_OF_RANGE_INDICES`
    const FIXTURE_BUFFER_VIEWS: &str = r#"[
        {"buffer":0,"byteOffset":0,"byteLength":36},
        {"buffer":0,"byteOffset":36,"byteLength":36},
        {"buffer":0,"byteOffset":72,"byteLength":6},
        {"buffer":0,"byteOffset":0,"byteLength":1200},
        {"buffer":0,"byteOffset":72,"byteLength":200},
        {"buffer":0,"byteOffset":78,"byteLength":12}
    ]"#;

    /// Names a case, then gives the file and a check that the error is the one expected
    type ErrorCase = (&'static str, Vec<u8>, fn(&GltfError) -> bool);

    /// Returns the bytes of `values`, each component as a little-endian `f32`
    fn encode_vec3s(values: &[[f32; 3]]) -> Vec<u8> {
        values
            .iter()
            .flatten()
            .flat_map(|component| component.to_le_bytes())
            .collect()
    }

    /// Returns the bytes of `values`, each as a little-endian `u16`
    fn encode_u16s(values: &[u16]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    /// Returns a fixture GLB file with the given `meshes` JSON, and the fixture accessors, buffer
    /// views and triangle data. The function pads the JSON with spaces, and the binary data with
    /// zeros
    fn build_fixture_glb(meshes: &str) -> Vec<u8> {
        let json = format!(
            r#"{{"meshes":{meshes},"accessors":{FIXTURE_ACCESSORS},"bufferViews":{FIXTURE_BUFFER_VIEWS}}}"#
        );
        let mut json_chunk = json.into_bytes();
        pad_to_four(&mut json_chunk, b' ');
        let mut binary = encode_vec3s(&TRIANGLE_POSITIONS);
        binary.extend(encode_vec3s(&TRIANGLE_NORMALS));
        binary.extend(encode_u16s(&TRIANGLE_INDICES));
        binary.extend(encode_u16s(&OUT_OF_RANGE_INDICES));
        pad_to_four(&mut binary, 0);
        build_glb(&json_chunk, &binary)
    }

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

    /// Checks that a file with a single triangle gives its positions, normals and indices
    #[test]
    fn read_mesh_with_single_triangle_returns_its_data() {
        let file = build_fixture_glb(TRIANGLE_MESHES);

        let mesh = read_mesh(&file).unwrap();

        assert_eq!(mesh.positions, TRIANGLE_POSITIONS, "positions");
        assert_eq!(mesh.normals, TRIANGLE_NORMALS, "normals");
        assert_eq!(mesh.indices, TRIANGLE_INDICES.map(u32::from), "indices");
    }

    /// Checks that the positions and normals come from the accessors that the attributes point to,
    /// rather than from the order of the accessors
    #[test]
    fn read_mesh_with_swapped_accessors_follows_attribute_names() {
        let file = build_fixture_glb(
            r#"[{"primitives":[{"attributes":{"POSITION":1,"NORMAL":0},"indices":2}]}]"#,
        );

        let mesh = read_mesh(&file).unwrap();

        assert_eq!(
            mesh.positions, TRIANGLE_NORMALS,
            "positions read through accessor 1"
        );
        assert_eq!(
            mesh.normals, TRIANGLE_POSITIONS,
            "normals read through accessor 0"
        );
    }

    /// Checks that a primitive whose accessors have a count of zero gives an empty mesh
    #[test]
    fn read_mesh_with_empty_primitive_returns_empty_data() {
        let file = build_fixture_glb(
            r#"[{"primitives":[{"attributes":{"POSITION":5,"NORMAL":5},"indices":6}]}]"#,
        );

        let mesh = read_mesh(&file).unwrap();

        assert!(
            mesh.positions.is_empty(),
            "positions of the empty primitive"
        );
        assert!(mesh.normals.is_empty(), "normals of the empty primitive");
        assert!(mesh.indices.is_empty(), "indices of the empty primitive");
    }

    /// Checks that only the first mesh is read, when a second one points to accessors that do not
    /// exist, or leaves out its `indices` and its `NORMAL` attribute
    #[test]
    fn read_mesh_with_two_meshes_reads_first() {
        let cases = [
            (
                "second mesh naming missing accessors",
                r#"[
                    {"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":2}]},
                    {"primitives":[{"attributes":{"POSITION":99,"NORMAL":99},"indices":99}]}
                ]"#,
            ),
            (
                "second mesh without indices or normals",
                r#"[
                    {"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":2}]},
                    {"primitives":[{"attributes":{"POSITION":0}}]}
                ]"#,
            ),
        ];

        for (name, meshes) in cases {
            let file = build_fixture_glb(meshes);

            let mesh = read_mesh(&file).unwrap();

            assert_eq!(
                mesh.positions, TRIANGLE_POSITIONS,
                "positions of the first mesh for {name}"
            );
            assert_eq!(
                mesh.normals, TRIANGLE_NORMALS,
                "normals of the first mesh for {name}"
            );
            assert_eq!(
                mesh.indices,
                TRIANGLE_INDICES.map(u32::from),
                "indices of the first mesh for {name}"
            );
        }
    }

    /// Checks that only the first primitive is read, when a second one points to accessors that
    /// do not exist
    #[test]
    fn read_mesh_with_two_primitives_reads_first() {
        let file = build_fixture_glb(
            r#"[{"primitives":[
                {"attributes":{"POSITION":0,"NORMAL":1},"indices":2},
                {"attributes":{"POSITION":99,"NORMAL":99},"indices":99}
            ]}]"#,
        );

        let mesh = read_mesh(&file).unwrap();

        assert_eq!(
            mesh.positions, TRIANGLE_POSITIONS,
            "positions of the first primitive"
        );
        assert_eq!(
            mesh.normals, TRIANGLE_NORMALS,
            "normals of the first primitive"
        );
        assert_eq!(
            mesh.indices,
            TRIANGLE_INDICES.map(u32::from),
            "indices of the first primitive"
        );
    }

    /// Checks that an empty list of meshes, or a first mesh without primitives, gives `MissingMesh`
    #[test]
    fn read_mesh_with_no_mesh_or_primitive_returns_missing_mesh() {
        let cases = [
            ("empty mesh list", "[]"),
            ("mesh without primitives", r#"[{"primitives":[]}]"#),
            (
                "first mesh without primitives",
                r#"[
                    {"primitives":[]},
                    {"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":2}]}
                ]"#,
            ),
        ];

        for (name, meshes) in cases {
            let file = build_fixture_glb(meshes);

            let result = read_mesh(&file);

            assert!(
                matches!(result, Err(GltfError::MissingMesh)),
                "result for {name}"
            );
        }
    }

    /// Checks that a valid glTF file that leaves out the optional `meshes` list gives `MissingMesh`
    #[test]
    fn read_mesh_with_no_meshes_key_returns_missing_mesh() {
        // Builds the smallest JSON that the glTF specification accepts. Since the specification
        // forbids an empty array, a file without meshes leaves the key out
        let file = build_glb(br#"{"asset":{"version":"2.0"}}"#, &[]);

        let result = read_mesh(&file);

        let error = result.err();
        assert!(
            matches!(error, Some(GltfError::MissingMesh)),
            "error for a file that leaves out the meshes key: {error:?}"
        );
    }

    /// Checks that an error from each stage of reading reaches the caller unchanged
    #[test]
    fn read_mesh_with_abnormal_files_returns_expected_error() {
        let mut wrong_magic = build_fixture_glb(TRIANGLE_MESHES);
        wrong_magic[0] = b'G';
        let mut cut_short = build_fixture_glb(TRIANGLE_MESHES);
        cut_short.truncate(cut_short.len() - 4);
        let mut version_one = build_fixture_glb(TRIANGLE_MESHES);
        version_one[4..8].copy_from_slice(&1_u32.to_le_bytes());
        let mut binary_chunk_first = build_fixture_glb(TRIANGLE_MESHES);
        binary_chunk_first[16..20].copy_from_slice(&0x004E_4942_u32.to_le_bytes());
        let mut length_one_more = build_fixture_glb(TRIANGLE_MESHES);
        let stated_length = length_one_more.len() as u32 + 1;
        length_one_more[8..12].copy_from_slice(&stated_length.to_le_bytes());

        let cases: [ErrorCase; 24] = [
            ("wrong magic", wrong_magic, |error| {
                matches!(error, GltfError::NotGlb)
            }),
            ("version 1", version_one, |error| {
                matches!(error, GltfError::UnsupportedVersion(1))
            }),
            (
                "BIN chunk type in place of JSON",
                binary_chunk_first,
                |error| {
                    matches!(
                        error,
                        GltfError::UnexpectedChunk {
                            expected: 0x4E4F_534A,
                            found: 0x004E_4942
                        }
                    )
                },
            ),
            ("empty file", Vec::new(), |error| {
                matches!(error, GltfError::Truncated)
            }),
            ("binary chunk cut short", cut_short, |error| {
                matches!(error, GltfError::Truncated)
            }),
            (
                "total length one byte more than the file",
                length_one_more,
                |error| matches!(error, GltfError::LengthMismatch { stated, actual } if *stated as usize == actual + 1),
            ),
            ("malformed JSON", build_glb(b"{", &[]), |error| {
                matches!(error, GltfError::Json(_))
            }),
            (
                "accessor index as text",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":"zero","NORMAL":1},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::Json(_)),
            ),
            (
                "missing position accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":99,"NORMAL":1},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::MissingAccessor(99)),
            ),
            (
                "missing normal accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":98},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::MissingAccessor(98)),
            ),
            (
                "missing indices accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":97}]}]"#,
                ),
                |error| matches!(error, GltfError::MissingAccessor(97)),
            ),
            (
                "primitive without normals",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::MissingNormals),
            ),
            (
                "primitive without indices",
                build_fixture_glb(r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1}}]}]"#),
                |error| matches!(error, GltfError::MissingIndices),
            ),
            (
                "unsupported position accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":3,"NORMAL":1},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::UnsupportedAccessor(3)),
            ),
            (
                "unsupported normal accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":2},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::UnsupportedAccessor(2)),
            ),
            (
                "unsupported indices accessor",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":0}]}]"#,
                ),
                |error| matches!(error, GltfError::UnsupportedAccessor(0)),
            ),
            (
                "positions past the binary chunk",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":4,"NORMAL":1},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::Truncated),
            ),
            (
                "normals past the binary chunk",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":4},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::Truncated),
            ),
            (
                "indices past the binary chunk",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":8}]}]"#,
                ),
                |error| matches!(error, GltfError::Truncated),
            ),
            (
                "missing buffer view",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":7,"NORMAL":1},"indices":2}]}]"#,
                ),
                |error| matches!(error, GltfError::MissingBufferView(9)),
            ),
            (
                "fewer normals than positions",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":9},"indices":2}]}]"#,
                ),
                |error| {
                    matches!(
                        error,
                        GltfError::NormalCountMismatch {
                            positions: 3,
                            normals: 2
                        }
                    )
                },
            ),
            (
                "more normals than positions",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":9,"NORMAL":1},"indices":2}]}]"#,
                ),
                |error| {
                    matches!(
                        error,
                        GltfError::NormalCountMismatch {
                            positions: 2,
                            normals: 3
                        }
                    )
                },
            ),
            (
                "index equal to the vertex count",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":10}]}]"#,
                ),
                |error| {
                    matches!(
                        error,
                        GltfError::IndexOutOfRange {
                            index: 3,
                            vertex_count: 3
                        }
                    )
                },
            ),
            (
                "largest 16-bit index",
                build_fixture_glb(
                    r#"[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":11}]}]"#,
                ),
                |error| {
                    matches!(
                        error,
                        GltfError::IndexOutOfRange {
                            index: 65_535,
                            vertex_count: 3
                        }
                    )
                },
            ),
        ];

        for (name, file, is_expected) in cases {
            let error = read_mesh(&file).err();

            assert!(
                error.as_ref().is_some_and(is_expected),
                "error for {name}: {error:?}"
            );
        }
    }
}
