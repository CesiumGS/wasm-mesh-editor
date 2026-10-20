use mesh_core::Mesh;
use slotmap::SlotMap;

use crate::{EditMode, MeshKey};

/// Active mesh access borrowed for a tool call. Tools retain keys, not mesh references.
pub struct ToolContext<'editor> {
    meshes: &'editor mut SlotMap<MeshKey, Mesh>,
    active_meshes: &'editor [MeshKey],
    mode: EditMode,
}

impl<'editor> ToolContext<'editor> {
    pub(crate) fn new(
        meshes: &'editor mut SlotMap<MeshKey, Mesh>,
        active_meshes: &'editor [MeshKey],
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

    pub fn active_meshes(&self) -> &'editor [MeshKey] {
        self.active_meshes
    }

    /// Returns an active mesh, or `None` if inactive or missing.
    pub fn mesh(&self, key: MeshKey) -> Option<&Mesh> {
        if !self.active_meshes.contains(&key) {
            return None;
        }

        self.meshes.get(key)
    }

    /// Returns an active mesh for editing, or `None` if inactive or missing.
    pub fn mesh_mut(&mut self, key: MeshKey) -> Option<&mut Mesh> {
        if !self.active_meshes.contains(&key) {
            return None;
        }

        self.meshes.get_mut(key)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tool/context.rs"]
mod tests;
