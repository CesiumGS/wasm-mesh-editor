//! Change events describing a single mesh mutation.

use std::collections::HashSet;
use std::rc::Rc;

use crate::{ComponentKey, ComponentType, EdgeKey, FaceKey, LoopKey, PerComponentType, VertKey};

pub enum MeshChange {
    Attributes(AttributeChange),
    Topology(TopologyChange),
    Selection(SelectionChange),
}

impl MeshChange {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Attributes(change) => change.is_empty(),
            Self::Topology(change) => change.is_empty(),
            Self::Selection(change) => change.is_empty(),
        }
    }
}

/// Unlike TopologyChange and SelectionChange, AttributeChange shares affected keys through `Rc`, avoiding per-frame key copies during
/// interactive edits such as translation (a hot path). Selection changes copy-on-write to these keys to preserve existing, unprocessed change events.
pub enum AttributeChange {
    Position(Rc<HashSet<VertKey>>),
    Normal(Rc<Vec<LoopKey>>),
    Uv(Rc<Vec<LoopKey>>),
}

impl AttributeChange {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Position(verts) => verts.is_empty(),
            Self::Normal(loops) | Self::Uv(loops) => loops.is_empty(),
        }
    }
}

pub struct TopologyChange {
    pub added_verts: Vec<VertKey>,
    pub removed_verts: Vec<VertKey>,
    pub added_edges: Vec<EdgeKey>,
    pub removed_edges: Vec<EdgeKey>,
    pub added_faces: Vec<FaceKey>,
    pub removed_faces: Vec<FaceKey>,
}

impl TopologyChange {
    pub fn is_empty(&self) -> bool {
        self.added_verts.is_empty()
            && self.removed_verts.is_empty()
            && self.added_edges.is_empty()
            && self.removed_edges.is_empty()
            && self.added_faces.is_empty()
            && self.removed_faces.is_empty()
    }
}

#[derive(Default)]
pub struct SelectionChange {
    pub added: PerComponentType<Vec<ComponentKey>>,
    pub removed: PerComponentType<Vec<ComponentKey>>,
}

impl SelectionChange {
    pub fn is_empty(&self) -> bool {
        [
            ComponentType::Vertex,
            ComponentType::Edge,
            ComponentType::Face,
        ]
        .into_iter()
        .all(|kind| self.added[kind].is_empty() && self.removed[kind].is_empty())
    }
}
