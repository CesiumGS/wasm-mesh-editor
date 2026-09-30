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
