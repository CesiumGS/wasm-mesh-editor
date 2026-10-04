//! Change events describing a single mesh mutation.

use crate::{AttributeDomain, ComponentKey, EdgeKey, FaceKey, LoopKey, PerComponentType, VertKey};

pub enum MeshChange {
    Attributes(AttributeChange),
    Topology(TopologyChange),
    Selection(SelectionChange),
}

pub struct AttributeChange {
    pub domain: AttributeDomain,
    pub verts: Vec<VertKey>,
    pub loops: Vec<LoopKey>,
}

pub struct TopologyChange {
    pub added_verts: Vec<VertKey>,
    pub removed_verts: Vec<VertKey>,
    pub added_edges: Vec<EdgeKey>,
    pub removed_edges: Vec<EdgeKey>,
    pub added_faces: Vec<FaceKey>,
    pub removed_faces: Vec<FaceKey>,
}

#[derive(Default)]
pub struct SelectionChange {
    pub added: PerComponentType<Vec<ComponentKey>>,
    pub removed: PerComponentType<Vec<ComponentKey>>,
}
