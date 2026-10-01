//! The [`Mesh`] aggregate: topology, attributes, and selection.

mod build;

use glam::Vec3;

use crate::selection::SelectionState;
use crate::{
    Attributes, EdgeKey, EdgeRef, FaceKey, FaceRef, ListenerId, MeshChange, Selection,
    SelectionView, Topology, VertKey, VertRef,
};

pub use build::{MeshBuffers, MeshBuildError};

/// A headless, editable mesh: connectivity, geometry attributes, and selection.
///
/// The three stores are separate fields so they can be borrowed disjointly
/// (e.g. `&mut attributes` alongside `&topology`).
/// The default is an empty mesh with selection at vertex level.
#[derive(Default)]
pub struct Mesh {
    pub(crate) topology: Topology,
    pub(crate) attributes: Attributes,
    pub(crate) selection: SelectionState,
}

impl Mesh {
    /// Creates an empty mesh with selection at vertex level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs an editable mesh from vertex attribute and face index buffers.
    /// Input buffers should contain decoded, typed data (not raw binary data).
    /// Vertices that are duplicated in the attribute buffers are treated as distinct vertices in the mesh.
    ///
    /// # Errors
    /// Returns an error if [`MeshBuffers::validate`] rejects the input buffers.
    pub fn from_buffers(buffers: MeshBuffers) -> Result<Self, MeshBuildError> {
        build::from_buffers(buffers)
    }

    pub fn topology(&self) -> &Topology {
        todo!()
    }

    pub fn attributes(&self) -> &Attributes {
        todo!()
    }

    pub fn vert(&self, key: VertKey) -> Option<VertRef<'_>> {
        todo!()
    }

    pub fn edge(&self, key: EdgeKey) -> Option<EdgeRef<'_>> {
        todo!()
    }

    /// Returns a reference to the face with the given key, if it exists.
    /// References wrap the underlying mesh component handle with access to both mesh topology and attributes.
    pub fn face(&self, key: FaceKey) -> Option<FaceRef<'_>> {
        if !self.topology.faces.contains_key(key) {
            return None;
        }
        Some(FaceRef {
            topo: &self.topology,
            attrs: &self.attributes,
            key,
        })
    }

    /// Translate the currently selected vertices by `delta`.
    pub fn translate_selected(&mut self, delta: Vec3) {
        todo!()
    }

    pub fn recompute_face_normals(&mut self, faces: &[FaceKey]) {
        todo!()
    }

    pub fn selection(&self) -> SelectionView<'_> {
        todo!()
    }

    pub fn selection_mut(&mut self) -> Selection<'_> {
        todo!()
    }

    pub fn add_change_listener(&mut self, listener: Box<dyn FnMut(&MeshChange)>) -> ListenerId {
        todo!()
    }

    pub fn remove_change_listener(&mut self, id: ListenerId) {
        todo!()
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh.rs"]
mod tests;
