//! Component handles and component-kind tags.

use slotmap::new_key_type;

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
    pub struct ComponentTypes: u8 {
        const VERTEX = 0b001;
        const EDGE = 0b010;
        const FACE = 0b100;
    }
}

impl From<ComponentType> for ComponentTypes {
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComponentKey {
    Vert(VertKey),
    Edge(EdgeKey),
    Face(FaceKey),
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
