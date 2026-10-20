use glam::DVec3;
use mesh::ComponentKey;
use slotmap::SecondaryMap;
use topology_overlay::{OverlayComponent, TopologyOverlay};

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
pub enum PickResult<Component = OverlayComponent> {
    /// No picking was requested for this event. This is not a miss.
    #[default]
    NotPerformed,
    /// Picking found no selectable component.
    Miss,
    Hit {
        mesh: MeshKey,
        component: Component,
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerInput<'input, Component = OverlayComponent> {
    pub modifiers: Modifiers,
    pub pick: PickResult<Component>,
    pub ray: Option<WorldRay>,
    pub scene: Option<SceneInput<'input>>,
}

// Written by hand: deriving would require `Component: Default`, which `ComponentKey` isn't.
impl<Component> Default for PointerInput<'_, Component> {
    fn default() -> Self {
        Self {
            modifiers: Modifiers::default(),
            pick: PickResult::NotPerformed,
            ray: None,
            scene: None,
        }
    }
}

impl<'input> PointerInput<'input> {
    /// Resolves the pick against the editor's overlays. Scene context stays borrowed.
    pub(crate) fn resolve(
        &self,
        overlays: &SecondaryMap<MeshKey, TopologyOverlay>,
    ) -> PointerInput<'input, ComponentKey> {
        PointerInput {
            modifiers: self.modifiers,
            pick: self.pick.resolve(overlays),
            ray: self.ray,
            scene: self.scene,
        }
    }
}

impl PickResult {
    fn resolve(
        self,
        overlays: &SecondaryMap<MeshKey, TopologyOverlay>,
    ) -> PickResult<ComponentKey> {
        match self {
            Self::NotPerformed => PickResult::NotPerformed,
            Self::Miss => PickResult::Miss,
            Self::Hit { mesh, component } => overlays
                .get(mesh)
                .and_then(|overlay| overlay.component_key(component))
                .map_or(PickResult::Miss, |component| PickResult::Hit {
                    mesh,
                    component,
                }),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tool/input.rs"]
mod tests;
