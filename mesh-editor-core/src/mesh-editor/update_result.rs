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
}
