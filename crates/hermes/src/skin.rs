use crate::GltfError;
use crate::model::find_buffer;
use gltf::{Gltf, Primitive, Skin};
use std::array;

// A matrix that changes nothing, as four columns of four floats
const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub struct SkinData {
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub bones: Vec<BoneData>,
}

pub struct BoneData {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub inverse_bind: [f32; 16],
}

fn find_parent_nodes(gltf: &Gltf) -> Vec<Option<usize>> {
    // Starts with no parent for every node in the file
    let mut parent_nodes = vec![None; gltf.nodes().len()];
    // Visits every node in the file
    for node in gltf.nodes() {
        // Visits every child of that node
        for child in node.children() {
            // Records the node as the parent of its child
            parent_nodes[child.index()] = Some(node.index());
        }
    }

    // Returns the parent of each node, indexed by node number
    parent_nodes
}

fn ensure_count(expected: usize, found: usize) -> Result<(), GltfError> {
    if expected != found {
        return Err(GltfError::SkinCountMismatch { expected, found });
    }

    // Accepts the list
    Ok(())
}

fn read_bones(gltf: &Gltf, skin: &Skin, binary: &[u8]) -> Result<Vec<BoneData>, GltfError> {
    // Lists the node number of each bone, in bone order
    let bone_nodes: Vec<usize> = skin.joints().map(|node| node.index()).collect();
    // Builds a reader, which follows the skin's accessor into the binary chunk
    let reader = skin.reader(|buffer| find_buffer(buffer, binary));
    // Reads one inverse bind matrix for each bone, as four columns of four floats
    let inverse_binds: Vec<[[f32; 4]; 4]> = match reader.read_inverse_bind_matrices() {
        // Collects the matrices that the file stores
        Some(matrices) => matrices.collect(),
        // Fills in the identity for every bone, which glTF means when it leaves the matrices out
        None => vec![IDENTITY; bone_nodes.len()],
    };
    // Checks that the file stores one matrix for each bone
    ensure_count(bone_nodes.len(), inverse_binds.len())?;

    // Works out the parent of every node in the file
    let parent_nodes = find_parent_nodes(gltf);
    // Makes room for one `BoneData` for each bone
    let mut bones = Vec::with_capacity(bone_nodes.len());
    // Walks the bones in bone order, each beside its inverse bind matrix
    for (bone_index, (node, columns)) in skin.joints().zip(inverse_binds).enumerate() {
        // Finds the node that is the bone's parent, if it has one
        let parent_node = parent_nodes[node.index()];
        // Turns that node number into a bone number, or `None` if the parent is not a bone
        let parent = parent_node.and_then(|parent_node| {
            // Finds where the parent's node sits in the list of bones
            bone_nodes
                .iter()
                .position(|&bone_node| bone_node == parent_node)
        });
        // The skinning code works out each parent before its children, so a parent must come first
        if parent.is_some_and(|parent| parent >= bone_index) {
            // Rejects the skin, stating the bone that comes too early
            return Err(GltfError::ParentAfterChild(bone_index));
        }

        // Copies the bone's name, or an empty name if the file gives none
        let name = node.name().unwrap_or_default().to_owned();
        // Splits the bone's place at rest into its translation, rotation and scale
        let (translation, rotation, scale) = node.transform().decomposed();
        // Lays the four columns of the matrix end to end, as sixteen floats
        let inverse_bind = array::from_fn(|index| columns[index / 4][index % 4]);
        // Records the bone
        bones.push(BoneData {
            name,
            parent,
            translation,
            rotation,
            scale,
            inverse_bind,
        });
    }

    // Returns every bone, in bone order
    Ok(bones)
}

pub(crate) fn read_skin(
    gltf: &Gltf,
    primitive: &Primitive,
    binary: &[u8],
    vertex_count: usize,
) -> Result<SkinData, GltfError> {
    // Builds a reader, which follows each accessor of the primitive into the binary chunk
    let reader = primitive.reader(|buffer| find_buffer(buffer, binary));
    // Reads the four bone numbers of each vertex, widening each one to a u16
    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .ok_or(GltfError::MissingSkin)?
        .into_u16()
        .collect();
    // Reads the four weights of each vertex, as floats
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .ok_or(GltfError::MissingSkin)?
        .into_f32()
        .collect();
    // Checks that every vertex has its bone numbers and its weights
    ensure_count(vertex_count, joints.len())?;
    ensure_count(vertex_count, weights.len())?;

    let skin = gltf.skins().next().ok_or(GltfError::MissingSkin)?; // Takes the first skin in the file
    let bones = read_bones(gltf, &skin, binary)?;
    let bone_count = bones.len();

    // Finds the first bone number that points at or past the last bone
    let out_of_range = joints
        .iter()
        .flatten()
        .find(|&&joint| usize::from(joint) >= bone_count);
    if let Some(&joint) = out_of_range {
        return Err(GltfError::JointOutOfRange { joint, bone_count });
    }

    // Returns the skin
    Ok(SkinData {
        joints,
        weights,
        bones,
    })
}

pub(crate) fn find_bone_nodes(gltf: &Gltf) -> Result<Vec<usize>, GltfError> {
    // Takes the first skin in the file
    let skin = gltf.skins().next().ok_or(GltfError::MissingSkin)?;
    // Returns the node number of each bone, in bone order
    Ok(skin.joints().map(|node| node.index()).collect())
}
