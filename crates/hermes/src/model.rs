use crate::accessor::{read_indices, read_vec3s};
use crate::document::Document;
use crate::glb::split_glb;
use crate::GltfError;

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

pub fn read_mesh(file: &[u8]) -> Result<MeshData, GltfError> {
    let glb = split_glb(file)?; // holds json and binary data
    let document: Document = serde_json::from_slice(glb.json)?;
    let mesh = document.meshes.first().ok_or(GltfError::MissingMesh)?;
    let primitive = mesh.primitives.first().ok_or(GltfError::MissingMesh)?;
    let positions = read_vec3s(&document, glb.binary, primitive.attributes.position)?;
    let normals = read_vec3s(&document, glb.binary, primitive.attributes.normal)?;
    let indices = read_indices(&document, glb.binary, primitive.indices)?;
    Ok(MeshData { positions, normals, indices })
}