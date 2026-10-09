//! Change events describing a single mesh mutation.

use std::collections::{HashMap, HashSet};
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

/// Selection events contain only net membership changes from a single operation.
#[derive(Default)]
pub struct SelectionChange {
    pub added: PerComponentType<Vec<ComponentKey>>,
    pub removed: PerComponentType<Vec<ComponentKey>>,
}

impl SelectionChange {
    /// Cancels opposing changes, including components changed more than once.
    pub fn normalize(&mut self) {
        let mut counts = HashMap::new();

        for kind in [
            ComponentType::Vertex,
            ComponentType::Edge,
            ComponentType::Face,
        ] {
            let added = &mut self.added[kind];
            let removed = &mut self.removed[kind];

            if added.is_empty() || removed.is_empty() {
                continue;
            }

            for key in added.drain(..) {
                *counts.entry(key).or_insert(0isize) += 1;
            }

            for key in removed.drain(..) {
                *counts.entry(key).or_insert(0isize) -= 1;
            }

            for (key, count) in counts.drain() {
                if count > 0 {
                    added.push(key);
                } else if count < 0 {
                    removed.push(key);
                }
            }
        }
    }

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
