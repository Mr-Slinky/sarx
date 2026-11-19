//! Holds the hand model, which the tests of several modules read.

/// Contains the bytes of the GLB file that Blender exported for the hand model
pub(crate) const HAND_MODEL: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/hand-model.glb"));
