//! The connectivity graph: the four component `SlotMap`s.

use slotmap::SlotMap;

use crate::{Edge, EdgeKey, Face, FaceKey, Loop, LoopKey, Vert, VertKey};

/// Stable-handle storage for the mesh's verts, edges, loops, and faces.
pub struct Topology {
    pub(crate) verts: SlotMap<VertKey, Vert>,
    pub(crate) edges: SlotMap<EdgeKey, Edge>,
    pub(crate) loops: SlotMap<LoopKey, Loop>,
    pub(crate) faces: SlotMap<FaceKey, Face>,
}

impl Topology {
    pub fn vert_count(&self) -> usize {
        todo!()
    }

    pub fn edge_count(&self) -> usize {
        todo!()
    }

    pub fn loop_count(&self) -> usize {
        todo!()
    }

    pub fn face_count(&self) -> usize {
        todo!()
    }

    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    pub fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        std::iter::empty::<LoopKey>()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }
}
