use glam::DVec3;
use mesh::ComponentKey;

use crate::{MeshKey, MeshTransform, WorldRay};

/// Keyboard modifiers at the time of the pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

/// The components picked on one mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickHit<'input> {
    pub mesh: MeshKey,
    pub components: &'input [ComponentKey],
}

/// A pick result for a pointer event
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickResult<'input> {
    /// No picking was requested for this event. This is not a miss.
    #[default]
    NotPerformed,
    /// The picked components, grouped by mesh. Empty if picking found nothing.
    /// Renderers may pick from buffers that can lag behind mesh edits, so hits may be stale.
    Hits(&'input [PickHit<'input>]),
}

/// Renderer-provided scene context
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneInput<'input> {
    /// Unit world-space camera direction, pointing into the scene.
    pub view_direction: DVec3,
    /// Scene placements of all meshes registered with the editor.
    pub mesh_transforms: &'input [MeshTransform],
}

/// Primary-pointer data resolved by the renderer. Defaults contain no pick or spatial data.
/// The renderer owns button mapping, click recognition, pointer capture, and camera controls.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerInput<'input> {
    pub modifiers: Modifiers,
    pub pick: PickResult<'input>,
    pub ray: Option<WorldRay>,
    pub scene: Option<SceneInput<'input>>,
}

#[cfg(test)]
#[path = "../../tests/unit/tool/input.rs"]
mod tests;
