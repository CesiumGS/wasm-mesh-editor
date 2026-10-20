//! Component handles, kinds, and composed component views.

use slotmap::new_key_type;

use crate::{Attributes, EdgeRef, FaceRef, Topology, VertRef};

new_key_type! {
    pub struct VertKey;
    pub struct EdgeKey;
    pub struct LoopKey;
    pub struct FaceKey;
}

/// A selectable component kind. Loops are internal and excluded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComponentType {
    Vertex,
    Edge,
    Face,
}

bitflags::bitflags! {
    /// A set of component kinds.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ComponentMask: u8 {
        const VERTEX = 0b001;
        const EDGE = 0b010;
        const FACE = 0b100;
    }
}

impl From<ComponentType> for ComponentMask {
    fn from(kind: ComponentType) -> Self {
        match kind {
            ComponentType::Vertex => Self::VERTEX,
            ComponentType::Edge => Self::EDGE,
            ComponentType::Face => Self::FACE,
        }
    }
}

/// A component handle tagged by kind, letting components of different kinds share
/// one flat collection.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ComponentKey {
    Vert(VertKey),
    Edge(EdgeKey),
    Face(FaceKey),
}

impl ComponentKey {
    /// The kind of component identified by this handle.
    pub fn kind(self) -> ComponentType {
        match self {
            Self::Vert(_) => ComponentType::Vertex,
            Self::Edge(_) => ComponentType::Edge,
            Self::Face(_) => ComponentType::Face,
        }
    }
}

/// A component view that can represent a vertex, edge, or face.
/// Obtained through [`crate::Mesh::component`].
pub enum ComponentRef<'a> {
    Vert(VertRef<'a>),
    Edge(EdgeRef<'a>),
    Face(FaceRef<'a>),
}

impl<'a> ComponentRef<'a> {
    pub(crate) fn new(key: ComponentKey, topo: &'a Topology, attrs: &'a Attributes) -> Self {
        match key {
            ComponentKey::Vert(key) => Self::Vert(VertRef { topo, attrs, key }),
            ComponentKey::Edge(key) => Self::Edge(EdgeRef { topo, key }),
            ComponentKey::Face(key) => Self::Face(FaceRef { topo, attrs, key }),
        }
    }

    /// Replaces `result` with immediate lower components: face->edges or edge->vertices.
    /// Vertices have no lower components.
    pub fn lower(&self, result: &mut Vec<ComponentKey>) {
        result.clear();
        match self {
            Self::Vert(_) => {}
            Self::Edge(edge) => result.extend(edge.verts().map(ComponentKey::Vert)),
            Self::Face(face) => result.extend(face.edges().map(ComponentKey::Edge)),
        }
    }

    /// Replaces `result` with immediate upper components: vertex->edges or edge->faces.
    /// Faces have no upper components.
    pub fn upper(&self, result: &mut Vec<ComponentKey>) {
        result.clear();
        match self {
            Self::Vert(vert) => result.extend(vert.edges().map(ComponentKey::Edge)),
            Self::Edge(edge) => result.extend(edge.faces().map(ComponentKey::Face)),
            Self::Face(_) => {}
        }
    }
}

impl From<VertKey> for ComponentKey {
    fn from(key: VertKey) -> Self {
        ComponentKey::Vert(key)
    }
}
impl From<EdgeKey> for ComponentKey {
    fn from(key: EdgeKey) -> Self {
        ComponentKey::Edge(key)
    }
}
impl From<FaceKey> for ComponentKey {
    fn from(key: FaceKey) -> Self {
        ComponentKey::Face(key)
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh_component.rs"]
mod tests;
