//! Shared selection helpers and drag-position tracking for tools.

use glam::DVec3;
use mesh_core::centroid;

use crate::{MeshTransform, ToolContext};

/// Copies placements for active meshes with selected vertices, reusing `output`.
/// A missing placement returns `None` and clears partial results.
pub(super) fn collect_active_mesh_transforms(
    context: &ToolContext<'_>,
    transforms: &[MeshTransform],
    output: &mut Vec<MeshTransform>,
) -> Option<()> {
    output.clear();

    for &key in context.active_meshes() {
        let mesh = context.mesh(key).expect("active mesh must exist");
        if mesh.selection().verts().next().is_none() {
            continue;
        }

        let Some(transform) = transforms.iter().find(|transform| transform.mesh == key) else {
            output.clear();
            return None;
        };
        output.push(*transform);
    }

    Some(())
}

/// Returns the world-space centroid of selected vertices, weighted by vertex count.
/// Every selected active mesh must have a placement.
pub(super) fn selection_centroid(
    context: &ToolContext<'_>,
    transforms: &[MeshTransform],
) -> Option<DVec3> {
    let mut center = DVec3::ZERO;
    let mut total_vertices = 0usize;

    for &key in context.active_meshes() {
        let mesh = context.mesh(key).expect("active mesh must exist");
        let selection = mesh.selection();
        let positions = selection
            .verts()
            .map(|vertex| mesh.attributes().positions[vertex].as_dvec3());
        let (local_centroid, vertex_count) = centroid(positions)?;

        if vertex_count == 0 {
            continue;
        }

        let transform = transforms.iter().find(|transform| transform.mesh == key)?;
        let world_centroid = transform.local_to_world().transform_point3(local_centroid);

        // Weight per-mesh means in each iteration instead of summing large world-space coordinates
        // after the loop (which is more prone to floating-point precision errors).
        total_vertices += vertex_count;
        center += (world_centroid - center) * (vertex_count as f64 / total_vertices as f64);
    }

    (total_vertices > 0 && center.is_finite()).then_some(center)
}

/// Drag-position history, independent of projection geometry and tool-specific edits.
pub(super) struct DragState {
    pub(super) start_position: DVec3,
    pub(super) current_position: DVec3,
    pub(super) last_update_position: DVec3,
}

impl DragState {
    pub(super) fn new(start_position: DVec3) -> Self {
        Self {
            start_position,
            current_position: start_position,
            last_update_position: start_position,
        }
    }

    /// Failed projections leave the pointer at its last valid position.
    pub(super) fn update_position(&mut self, position: Option<DVec3>) {
        if let Some(position) = position {
            self.current_position = position;
        }
    }

    /// Advances the baseline and returns movement since the last tool update.
    pub(super) fn update(&mut self) -> DVec3 {
        let delta = self.current_position - self.last_update_position;
        self.last_update_position = self.current_position;
        delta
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tool/common.rs"]
mod tests;
