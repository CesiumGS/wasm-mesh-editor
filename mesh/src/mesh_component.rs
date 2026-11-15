//! Component handles, kinds, and composed component views.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use slotmap::{Key, KeyData, new_key_type};

use crate::{Attributes, EdgeRef, FaceRef, Topology, VertRef};

/// Mirrors SlotMap's `new_key_type!` macro, but explicitly stores the key's `KeyData::as_ffi` value so slices of keys can be
/// shared across FFI as `u64`s without any conversion from SlotMap's key type, to save on overhead.
macro_rules! ffi_key_type {
    ($($(#[$meta:meta])* $vis:vis struct $name:ident;)*) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(transparent)]
        $vis struct $name(u64);

        const _: () = assert!(size_of::<$name>() == 8 && align_of::<$name>() == 8);

        impl $name {
            /// Returns the key whose `KeyData::as_ffi` value is `id`, or `None` if no slot map
            /// could have issued it.
            pub const fn try_from_ffi(id: u64) -> Option<Self> {
                // Slot maps only issue odd versions, and `from_ffi` forces the version to be odd. An even version
                // would decode as a different key, so its equality would no longer match `KeyData`'s. This check ensures we only accept valid FFI keys.
                if (id >> 32) & 1 == 1 { Some(Self(id)) } else { None }
            }
        }

        impl From<KeyData> for $name {
            fn from(data: KeyData) -> Self {
                Self(data.as_ffi())
            }
        }

        // SAFETY: slotmap requires our keys to behave exactly like their `KeyData`:
        // - The private `u64` only holds `KeyData::as_ffi` values, which `from_ffi` decodes exactly.
        // - `as_ffi` is one-to-one, so derived equality on the `u64` matches `KeyData`'s.
        // - `Ord`, `Hash`, `Debug`, and `Default` delegate to `KeyData`.
        unsafe impl Key for $name {
            fn data(&self) -> KeyData {
                KeyData::from_ffi(self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                KeyData::default().into()
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> Ordering {
                self.data().cmp(&other.data())
            }
        }

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.data().hash(state);
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.data().fmt(f)
            }
        }
    )*};
}

ffi_key_type! {
    pub struct VertKey;
    pub struct EdgeKey;
    pub struct FaceKey;
}

new_key_type! {
    pub struct LoopKey;
}

/// A selectable component kind. Loops are internal and excluded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
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

/// Component keys grouped by kind.
#[derive(Clone, Default)]
pub struct ComponentKeys {
    pub verts: Vec<VertKey>,
    pub edges: Vec<EdgeKey>,
    pub faces: Vec<FaceKey>,
}

impl ComponentKeys {
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

/// A set of vertices, edges, and faces.
#[derive(Default)]
pub(crate) struct ComponentSet {
    /// `Rc` so the vertex set can be shared without copying.
    pub(crate) verts: Rc<HashSet<VertKey>>,
    pub(crate) edges: HashSet<EdgeKey>,
    pub(crate) faces: HashSet<FaceKey>,
}

impl ComponentSet {
    pub(crate) fn contains(&self, key: ComponentKey) -> bool {
        match key {
            ComponentKey::Vert(key) => self.verts.contains(&key),
            ComponentKey::Edge(key) => self.edges.contains(&key),
            ComponentKey::Face(key) => self.faces.contains(&key),
        }
    }

    /// Returns whether `key` was added.
    pub(crate) fn insert(&mut self, key: ComponentKey) -> bool {
        match key {
            // Checked first so a shared vertex set isn't copied when nothing changes.
            ComponentKey::Vert(key) => {
                !self.verts.contains(&key) && Rc::make_mut(&mut self.verts).insert(key)
            }
            ComponentKey::Edge(key) => self.edges.insert(key),
            ComponentKey::Face(key) => self.faces.insert(key),
        }
    }

    /// Returns whether `key` was removed.
    pub(crate) fn remove(&mut self, key: ComponentKey) -> bool {
        match key {
            // Checked first so a shared vertex set isn't copied when nothing changes.
            ComponentKey::Vert(key) => {
                self.verts.contains(&key) && Rc::make_mut(&mut self.verts).remove(&key)
            }
            ComponentKey::Edge(key) => self.edges.remove(&key),
            ComponentKey::Face(key) => self.faces.remove(&key),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.verts.len() + self.edges.len() + self.faces.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every component, in unspecified order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = ComponentKey> + '_ {
        self.verts
            .iter()
            .copied()
            .map(ComponentKey::Vert)
            .chain(self.edges.iter().copied().map(ComponentKey::Edge))
            .chain(self.faces.iter().copied().map(ComponentKey::Face))
    }

    /// Empties the set, returning the removed keys.
    pub(crate) fn take(&mut self) -> ComponentKeys {
        let verts = match Rc::get_mut(&mut self.verts) {
            Some(verts) => verts.drain().collect(),
            // Leaves the shared set intact for its other owners.
            None => std::mem::take(&mut self.verts).iter().copied().collect(),
        };

        ComponentKeys {
            verts,
            edges: self.edges.drain().collect(),
            faces: self.faces.drain().collect(),
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
            ComponentKey::Edge(key) => Self::Edge(EdgeRef { topo, attrs, key }),
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
