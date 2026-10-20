use super::MeshKey;

/// Reusable output from [`super::MeshEditor::update`].
#[derive(Debug, Default)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct UpdateResult {
    /// Meshes whose overlay buffers changed during this update.
    #[cfg_attr(
        feature = "wasm",
        wasm_bindgen(readonly, getter_with_clone, js_name = changedOverlays)
    )]
    pub changed_overlays: Vec<MeshKey>,
    /// Meshes that became active since the previous update, by a tool or the consumer.
    pub activated_meshes: Vec<MeshKey>,
    /// Meshes that became inactive since the previous update, including active meshes that
    /// were removed. A mesh whose activation was undone before the update is in neither list.
    pub deactivated_meshes: Vec<MeshKey>,
}
