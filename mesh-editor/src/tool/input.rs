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

/// A pick result for a pointer event
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickResult {
    /// No picking was requested for this event. This is not a miss.
    #[default]
    NotPerformed,
    /// Picking found no selectable component.
    Miss,
    /// Tools receive hits on missing meshes / components as misses.
    Hit {
        mesh: MeshKey,
        component: ComponentKey,
    },
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
    pub pick: PickResult,
    pub ray: Option<WorldRay>,
    pub scene: Option<SceneInput<'input>>,
}

#[cfg(test)]
#[path = "../../tests/unit/tool/input.rs"]
mod tests;
