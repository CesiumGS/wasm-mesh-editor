//! Bounds of the selected and unselected vertices.

use std::sync::OnceLock;

use crate::Aabb;

/// Lazy bounding boxes whose union is the mesh bounds. Translating the selection
/// shifts the selected box and leaves the unselected box unchanged.
#[derive(Default)]
pub(super) struct BoundsCache {
    pub(super) selected: OnceLock<Option<Aabb>>,
    pub(super) unselected: OnceLock<Option<Aabb>>,
}
