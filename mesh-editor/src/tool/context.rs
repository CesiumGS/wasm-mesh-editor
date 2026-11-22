use mesh::Mesh;
use slotmap::SlotMap;

use crate::mesh_editor::ActiveMeshes;
use crate::{EditMode, MeshKey, MeshNotFound};

/// Active mesh access borrowed for a tool call. Tools retain keys, not mesh references.
pub struct ToolContext<'editor> {
    meshes: &'editor mut SlotMap<MeshKey, Mesh>,
    active_meshes: &'editor mut ActiveMeshes,
    mode: EditMode,
}

impl<'editor> ToolContext<'editor> {
    pub(crate) fn new(
        meshes: &'editor mut SlotMap<MeshKey, Mesh>,
        active_meshes: &'editor mut ActiveMeshes,
        mode: EditMode,
    ) -> Self {
        Self {
            meshes,
            active_meshes,
            mode,
        }
    }

    pub fn mode(&self) -> EditMode {
        self.mode
    }

    pub fn active_meshes(&self) -> &[MeshKey] {
        self.active_meshes.keys()
    }

    pub fn is_mesh_active(&self, key: MeshKey) -> bool {
        self.active_meshes.contains(key)
    }

    /// Returns an active mesh, or `None` if inactive or missing.
    pub fn mesh(&self, key: MeshKey) -> Option<&Mesh> {
        if !self.is_mesh_active(key) {
            return None;
        }

        self.meshes.get(key)
    }

    /// Returns an active mesh for editing, or `None` if inactive or missing.
    pub fn mesh_mut(&mut self, key: MeshKey) -> Option<&mut Mesh> {
        if !self.is_mesh_active(key) {
            return None;
        }

        self.meshes.get_mut(key)
    }

    /// Returns all active meshes for editing, in unspecified order.
    pub fn active_meshes_mut(&mut self) -> impl Iterator<Item = &mut Mesh> {
        let active_meshes = &*self.active_meshes;

        self.meshes
            .iter_mut()
            .filter(move |(key, _)| active_meshes.contains(*key))
            .map(|(_, mesh)| mesh)
    }

    /// Like [`crate::MeshEditor::set_mesh_active`], but does not cancel the active tool.
    pub fn set_mesh_active(&mut self, key: MeshKey) -> Result<(), MeshNotFound> {
        self.active_meshes.activate(self.meshes, self.mode, key)
    }

    /// Keeps only the active meshes for which `keep` returns `true`, preserving their order.
    /// Deactivated meshes keep their selection and selection level.
    pub fn retain_active_meshes(&mut self, keep: impl FnMut(&MeshKey) -> bool) {
        self.active_meshes.retain(keep);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tool/context.rs"]
mod tests;
