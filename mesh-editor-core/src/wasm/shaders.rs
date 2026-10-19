//! JS adapters for [`topology_overlay::shaders`].

use serde::Serialize;
use topology_overlay::shaders;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

// Doc comments on these JS-facing types also document the generated TypeScript.

/// GLSL ES 3.00 sources of one program. Build each stage as `#version 300 es`, then the
/// program's source, then the hook definitions.
#[derive(Serialize, Tsify)]
pub struct ShaderSources {
    vertex: &'static str,
    fragment: &'static str,
}

impl From<shaders::ShaderSources> for ShaderSources {
    fn from(sources: shaders::ShaderSources) -> Self {
        Self {
            vertex: sources.vertex,
            fragment: sources.fragment,
        }
    }
}

/// Shaders that draw the buffers from `overlayViews` as WebGL2 data textures.
///
/// Each program owns `main()` and calls hooks that the consumer defines:
/// - vertex: `vec4 topologyOverlay_modelToEye(vec3 positionMC)`,
///   `vec4 topologyOverlay_eyeToClip(vec4 positionEC)`, `vec2 topologyOverlay_viewportSize()`,
///   and `void topologyOverlay_setPickOutputs(uint componentIndex)`
/// - fragment: `void topologyOverlay_writeColor(vec4 color)`
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct TopologyOverlayShaders {
    /// Faces: `triangles.elementCount` instances of 3 vertices each, drawn as `TRIANGLES`.
    face: ShaderSources,
    /// Hooks for plain WebGL2, driven by `u_modelView`, `u_projection`, and `u_viewportSize`.
    /// They don't encode picks.
    default_hooks: ShaderSources,
}

/// Returns the overlay's shaders. They never change, so read them once.
#[wasm_bindgen(js_name = topologyOverlayShaders)]
pub fn topology_overlay_shaders() -> Result<Ts<TopologyOverlayShaders>, JsError> {
    Ok(TopologyOverlayShaders {
        face: shaders::FACE.into(),
        default_hooks: shaders::DEFAULT_HOOKS.into(),
    }
    .into_ts()?)
}
