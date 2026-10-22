use super::MeshKey;

/// Reusable output from [`super::MeshEditor::update`].
#[derive(Debug, Default)]
pub struct UpdateResult {
    /// Meshes whose overlay buffers changed during this update.
    pub changed_overlays: Vec<MeshKey>,
}
