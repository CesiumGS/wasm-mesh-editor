use mesh_core::Mesh;
use slotmap::SlotMap;

use crate::{EditMode, MeshKey};

/// Active mesh access borrowed for a tool call. Tools retain keys, not mesh references.
#[allow(dead_code)]
pub struct ToolContext<'editor> {
    meshes: &'editor mut SlotMap<MeshKey, Mesh>,
    active_meshes: &'editor [MeshKey],
    mode: EditMode,
}

#[allow(unused_variables)]
impl<'editor> ToolContext<'editor> {
    #[allow(dead_code)]
    pub(crate) fn new(
        meshes: &'editor mut SlotMap<MeshKey, Mesh>,
        active_meshes: &'editor [MeshKey],
        mode: EditMode,
    ) -> Self {
        todo!()
    }

    pub fn mode(&self) -> EditMode {
        todo!()
    }

    pub fn active_meshes(&self) -> &'editor [MeshKey] {
        todo!()
    }

    /// Returns an active mesh, or `None` if inactive or missing.
    pub fn mesh(&self, key: MeshKey) -> Option<&Mesh> {
        todo!()
    }

    /// Returns an active mesh for editing, or `None` if inactive or missing.
    pub fn mesh_mut(&mut self, key: MeshKey) -> Option<&mut Mesh> {
        todo!()
    }
}
