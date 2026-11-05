use mesh::Mesh;
use slotmap::SlotMap;

use super::{MeshKey, MeshNotFound};
use crate::EditMode;

/// The ordered set of active meshes. All activation changes pass through here so they can be
/// recorded and reported by [`super::MeshEditor::update`].
#[derive(Debug, Default)]
pub(crate) struct ActiveMeshes {
    keys: Vec<MeshKey>,
    activated: Vec<MeshKey>,
    deactivated: Vec<MeshKey>,
}

impl ActiveMeshes {
    pub(crate) fn keys(&self) -> &[MeshKey] {
        &self.keys
    }

    pub(crate) fn contains(&self, key: MeshKey) -> bool {
        self.keys.contains(&key)
    }

    /// Applies `mode`'s selection level to the mesh and appends it to the active set.
    /// Does nothing if the mesh is already active. Returns [`MeshNotFound`] for an unknown key.
    pub(crate) fn activate(
        &mut self,
        meshes: &mut SlotMap<MeshKey, Mesh>,
        mode: EditMode,
        key: MeshKey,
    ) -> Result<(), MeshNotFound> {
        let mesh = meshes.get_mut(key).ok_or(MeshNotFound { key })?;

        if self.contains(key) {
            return Ok(());
        }

        mesh.selection_mut().set_level(mode.selection_level);
        self.keys.push(key);
        Self::record(&mut self.activated, &mut self.deactivated, key);

        Ok(())
    }

    /// Removes a mesh from the active set. Returns `false` if it was not active.
    pub(crate) fn deactivate(&mut self, key: MeshKey) -> bool {
        let Some(index) = self.keys.iter().position(|&active| active == key) else {
            return false;
        };

        self.keys.remove(index);
        Self::record(&mut self.deactivated, &mut self.activated, key);

        true
    }

    /// Keeps only the meshes for which `keep` returns `true`, preserving their order.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&MeshKey) -> bool) {
        let Self {
            keys,
            activated,
            deactivated,
        } = self;

        keys.retain(|key| {
            let kept = keep(key);

            if !kept {
                Self::record(deactivated, activated, *key);
            }

            kept
        });
    }

    /// Replaces `activated` and `deactivated` with the changes recorded since the last call, in
    /// the order they first occurred. A mesh activated and deactivated in between (or vice versa)
    /// is not reported.
    ///
    /// Swaps buffers instead of copying keys; the caller's cleared buffers record the next changes.
    pub(crate) fn take_changes(
        &mut self,
        activated: &mut Vec<MeshKey>,
        deactivated: &mut Vec<MeshKey>,
    ) {
        activated.clear();
        deactivated.clear();
        std::mem::swap(activated, &mut self.activated);
        std::mem::swap(deactivated, &mut self.deactivated);
    }

    /// Records a change, or cancels the opposite change if one is still unreported.
    fn record(changes: &mut Vec<MeshKey>, opposite_changes: &mut Vec<MeshKey>, key: MeshKey) {
        match opposite_changes.iter().position(|&changed| changed == key) {
            Some(index) => {
                opposite_changes.remove(index);
            }
            None => changes.push(key),
        }
    }
}

/// Seeds an active set without recording any changes.
#[cfg(test)]
impl From<Vec<MeshKey>> for ActiveMeshes {
    fn from(keys: Vec<MeshKey>) -> Self {
        Self {
            keys,
            ..Self::default()
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/mesh-editor/active_meshes.rs"]
mod tests;
