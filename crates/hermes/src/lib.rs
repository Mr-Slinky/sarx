//! Reads a model from a binary glTF file, and returns it in Sarx's own types.
//!
//! Blender exports the god's hand as `assets/hand-model.glb`. [`read_model`] reads the whole
//! model: the mesh that gives the hand its shape, the skin that ties the mesh to a skeleton of
//! bones, and the poses that the hand moves between. [`read_mesh`] reads the mesh alone. The
//! example on [`read_model`] reads the hand and looks at each of the three parts.
//!
//! The crate reads the file through the `gltf` crate, as ADR 0010 sets out. Every public type here
//! is Sarx's own, apart from the error that [`GltfError::Gltf`] wraps.
//!
//! # The file
//!
//! A binary glTF file contains a JSON text, which describes the model, and a block of bytes, which
//! stores its numbers. Each list of numbers, such as the positions of the vertices, has an
//! accessor in the JSON text. The accessor states where the list lies in the block of bytes, and
//! what type of number each value is. The `gltf` crate follows each accessor, and this crate copies
//! the values into its own types.
//!
//! A glTF file can contain several meshes, and each mesh can contain several primitives. A
//! primitive is one set of triangles that the GPU draws in a single call. The crate reads the first
//! primitive of the first mesh, and the first skin. It reads only buffers inside the `.glb` file
//! itself, and a list that lies in a separate file counts as missing.
//!
//! # Mesh
//!
//! [`MeshData`] gives the shape of the model as a list of vertices and a list of triangles. Each
//! vertex has a position and a normal. The normal is the direction that the surface faces at that
//! vertex, which lighting needs. Each triangle takes three entries in `indices`, and each entry is
//! a vertex number, which selects one vertex from the list. As a result, neighbouring triangles
//! share a vertex rather than each storing a copy.
//!
//! # Skin
//!
//! [`SkinData`] ties the mesh to a skeleton. Each vertex follows up to four bones, and blends their
//! movements by weight. For each vertex, `joints` gives the numbers of its four bones, and
//! `weights` gives how strongly the vertex follows each one.
//!
//! [`BoneData`] describes one bone: its parent, its place at rest, and its inverse bind matrix. The
//! inverse bind matrix undoes the bone's place at rest. It moves a vertex from the space of the
//! model into the space of the bone, where skinning code can then apply the bone's current place.
//!
//! Every bone comes after its parent in the list of bones. Code that walks the list in order
//! therefore meets each parent before its children.
//!
//! # Poses
//!
//! A pose gives each bone of the hand a rotation, as ADR 0020 sets out. Blender stores each pose as
//! an animation with a single keyframe, and [`PoseData`] stores the rotations from that keyframe.
//! The crate leaves out any animation that gives a bone more or fewer than one keyframe of
//! rotation.
//!
//! Each rotation is a quaternion: four numbers that together store a rotation, in the order
//! `[x, y, z, w]`.

mod error;
mod model;
mod pose;
mod skin;
#[cfg(test)]
mod test_support;

pub use error::GltfError;
pub use model::{MeshData, Model, read_mesh, read_model};
pub use pose::PoseData;
pub use skin::{BoneData, SkinData};