use glam::{DMat4, DVec3};
use mesh_core::ComponentKey;

use crate::MeshKey;

/// Keyboard modifiers at the time of the pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

/// A renderer-resolved pick. Tools validate the mesh and component before use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickResult {
    /// No picking was requested for this event. This is not a miss.
    #[default]
    NotPerformed,
    /// Picking found no selectable component.
    Miss,
    Hit {
        mesh: MeshKey,
        component: ComponentKey,
    },
}

/// A world-space pointer ray with a finite origin and unit-length direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldRay {
    pub origin: DVec3,
    pub direction: DVec3,
}

/// A mesh's scene placement, supplied by the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshTransform {
    pub mesh: MeshKey,
    /// Finite, invertible affine transform from mesh-local to world coordinates.
    pub local_to_world: DMat4,
}

/// Spatial context supplied only when starting a drag.
/// Tools retain the per-mesh values they need, not this borrowed slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragStartInput<'input> {
    /// Unit world-space camera direction, pointing into the scene.
    pub view_direction: DVec3,
    /// One entry per active mesh with selected vertices, including identity transforms.
    /// Keys must be unique; missing transforms do not imply identity.
    pub mesh_transforms: &'input [MeshTransform],
}

/// Primary-pointer data resolved by the renderer. Defaults contain no pick or spatial data.
/// The renderer owns button mapping, click recognition, pointer capture, and camera controls.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerInput<'input> {
    pub modifiers: Modifiers,
    pub pick: PickResult,
    pub ray: Option<WorldRay>,
    pub drag_start: Option<DragStartInput<'input>>,
}

#[cfg(test)]
#[path = "../../tests/unit/tool/input.rs"]
mod tests;
