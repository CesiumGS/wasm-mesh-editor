//! The [`Mesh`] aggregate: topology, attributes, and selection.

use glam::{Vec2, Vec3};

use crate::selection::SelectionState;
use crate::{
    Attributes, EdgeKey, EdgeRef, FaceKey, FaceRef, ListenerId, MeshChange, Selection,
    SelectionView, Topology, VertKey, VertRef,
};

/// Buffers describing a mesh to build.
pub struct MeshOptions {
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub uvs: Option<Vec<Vec2>>,
    pub indices: Vec<u32>,
    /// Vertices per face, for n-gons; triangles are assumed when absent.
    pub face_vertex_counts: Option<Vec<u32>>,
}

/// A headless, editable mesh: connectivity, geometry attributes, and selection.
///
/// The three stores are separate fields so they can be borrowed disjointly
/// (e.g. `&mut attributes` alongside `&topology`).
pub struct Mesh {
    pub(crate) topology: Topology,
    pub(crate) attributes: Attributes,
    pub(crate) selection: SelectionState,
}

impl Mesh {
    pub fn from_options(options: MeshOptions) -> Self {
        todo!()
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

    pub fn face(&self, key: FaceKey) -> Option<FaceRef<'_>> {
        todo!()
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
