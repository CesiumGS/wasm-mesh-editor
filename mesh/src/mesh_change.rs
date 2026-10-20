//! Change events describing a single mesh mutation.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use slotmap::{Key, KeyData};

use crate::{ComponentKey, ComponentType, EdgeKey, FaceKey, LoopKey, VertKey};

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

/// Handles grouped by component kind for selection changes.
#[derive(Clone, Default)]
pub struct SelectionKeys {
    pub verts: Vec<VertKey>,
    pub edges: Vec<EdgeKey>,
    pub faces: Vec<FaceKey>,
}

impl SelectionKeys {
    /// Appends a handle to its component list.
    pub fn push(&mut self, key: ComponentKey) {
        match key {
            ComponentKey::Vert(key) => self.verts.push(key),
            ComponentKey::Edge(key) => self.edges.push(key),
            ComponentKey::Face(key) => self.faces.push(key),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }

    /// The number of handles of this kind.
    pub fn len(&self, kind: ComponentType) -> usize {
        match kind {
            ComponentType::Vertex => self.verts.len(),
            ComponentType::Edge => self.edges.len(),
            ComponentType::Face => self.faces.len(),
        }
    }

    /// A handle from this kind's list, or `None` if out of bounds.
    pub fn get(&self, kind: ComponentType, index: usize) -> Option<ComponentKey> {
        match kind {
            ComponentType::Vertex => self.verts.get(index).copied().map(ComponentKey::Vert),
            ComponentType::Edge => self.edges.get(index).copied().map(ComponentKey::Edge),
            ComponentType::Face => self.faces.get(index).copied().map(ComponentKey::Face),
        }
    }

    /// Handles of this kind in insertion order.
    pub fn iter(&self, kind: ComponentType) -> impl ExactSizeIterator<Item = ComponentKey> + '_ {
        (0..self.len(kind)).map(move |index| self.get(kind, index).unwrap())
    }
}

/// Selection events contain only net membership changes from a single operation.
#[derive(Default)]
pub struct SelectionChange {
    pub added: SelectionKeys,
    pub removed: SelectionKeys,
}

impl SelectionChange {
    /// Cancels opposing changes, including components changed more than once.
    pub fn normalize(&mut self) {
        let mut counts = HashMap::new();

        Self::normalize_keys(&mut self.added.verts, &mut self.removed.verts, &mut counts);
        Self::normalize_keys(&mut self.added.edges, &mut self.removed.edges, &mut counts);
        Self::normalize_keys(&mut self.added.faces, &mut self.removed.faces, &mut counts);
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    fn normalize_keys<K: Key>(
        added: &mut Vec<K>,
        removed: &mut Vec<K>,
        counts: &mut HashMap<KeyData, isize>,
    ) {
        if added.is_empty() || removed.is_empty() {
            return;
        }

        for key in added.drain(..) {
            *counts.entry(key.data()).or_default() += 1;
        }

        for key in removed.drain(..) {
            *counts.entry(key.data()).or_default() -= 1;
        }

        for (key, count) in counts.drain() {
            if count > 0 {
                added.push(K::from(key));
            } else if count < 0 {
                removed.push(K::from(key));
            }
        }
    }
}
