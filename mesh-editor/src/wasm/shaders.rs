//! JS adapters for [`topology_overlay::shaders`].

use serde::Serialize;
use topology_overlay::shaders::{self, uniforms};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

// Doc comments on these JS-facing types also document the generated TypeScript.

/// GLSL ES 3.00 sources of one shader. Build each stage as `#version 300 es`, then the
/// shader's source, then the hook definitions.
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

/// The point shader's style uniforms.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PointUniforms {
    /// `float`: diameter in pixels.
    size: &'static str,
    /// `vec4`: color of unselected points.
    color: &'static str,
    /// `vec4`: color of selected points.
    selected_color: &'static str,
}

/// The edge shader's style uniforms.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EdgeUniforms {
    /// `float`: width in pixels.
    width: &'static str,
    /// `vec4`: color of unselected edges.
    color: &'static str,
    /// `vec4`: color of selected edges.
    selected_color: &'static str,
}

/// The face shader's style uniforms.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct FaceUniforms {
    /// `vec4`: color of unselected faces. Often transparent.
    color: &'static str,
    /// `vec4`: color of selected faces.
    selected_color: &'static str,
    /// `bool`: draws every face opaque and unshaded, so transparent faces stay pickable in a
    /// pick pass.
    is_pick_pass: &'static str,
}

/// The view's uniforms, declared by the default hooks. Consumers with their own hooks don't
/// set them.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ViewUniforms {
    /// `mat4`: model to eye space.
    model_view: &'static str,
    /// `mat4`: eye to clip space.
    projection: &'static str,
    /// `vec2`: viewport size in pixels.
    viewport_size: &'static str,
}

/// Names of the shaders' uniforms, other than the buffer textures: those are each buffer's
/// `layout.uniformName` in `overlayViews`.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct TopologyOverlayUniforms {
    /// `float`, in every shader: pulls the overlay toward the camera by this fraction of its
    /// eye-space depth. Set per draw, e.g. larger for points than edges, and edges than faces,
    /// so points draw over edges and edges over faces.
    depth_bias: &'static str,
    point: PointUniforms,
    edge: EdgeUniforms,
    face: FaceUniforms,
    view: ViewUniforms,
}

/// Shaders that draw the buffers from `overlayViews` as WebGL2 data textures.
///
/// Upload each buffer as a texture with `NEAREST` min and mag filters: the default filters leave
/// data textures incomplete, and the shaders then read zeros.
///
/// Each shader owns `main()` and calls hooks that the consumer defines:
/// - vertex: `vec4 topologyOverlay_modelToEye(vec3 positionMC)`,
///   `vec4 topologyOverlay_eyeToClip(vec4 positionEC)`, `vec2 topologyOverlay_viewportSize()`,
///   and `void topologyOverlay_setVertexOutputs(vec4 positionEC, uint componentIndex)`.
///   `eyeToClip` must be linear, like a projection matrix, and free of side effects: it may be
///   called more than once per vertex. `setVertexOutputs` is called once per vertex after
///   `gl_Position` is final, for outputs such as custom depth and pick colors.
/// - fragment: `void topologyOverlay_writeColor(vec4 color)`
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct TopologyOverlayShaders {
    /// Points: `positions.elementCount` instances of 4 vertices each, drawn as `TRIANGLE_STRIP`.
    point: ShaderSources,
    /// Edges: `edges.elementCount` instances of 4 vertices each, drawn as `TRIANGLE_STRIP`.
    edge: ShaderSources,
    /// Faces: `triangles.elementCount` instances of 3 vertices each, drawn as `TRIANGLES`.
    face: ShaderSources,
    /// Hooks for plain WebGL2, driven by `uniforms.view`. They don't encode picks.
    default_hooks: ShaderSources,
    uniforms: TopologyOverlayUniforms,
}

/// Returns the overlay's shaders. They never change, so read them once.
#[wasm_bindgen(js_name = topologyOverlayShaders)]
pub fn topology_overlay_shaders() -> Result<Ts<TopologyOverlayShaders>, JsError> {
    Ok(TopologyOverlayShaders {
        point: shaders::POINT.into(),
        edge: shaders::EDGE.into(),
        face: shaders::FACE.into(),
        default_hooks: shaders::DEFAULT_HOOKS.into(),
        uniforms: TopologyOverlayUniforms {
            depth_bias: uniforms::DEPTH_BIAS,
            point: PointUniforms {
                size: uniforms::POINT_SIZE,
                color: uniforms::POINT_COLOR,
                selected_color: uniforms::POINT_SELECTED_COLOR,
            },
            edge: EdgeUniforms {
                width: uniforms::EDGE_WIDTH,
                color: uniforms::EDGE_COLOR,
                selected_color: uniforms::EDGE_SELECTED_COLOR,
            },
            face: FaceUniforms {
                color: uniforms::FACE_COLOR,
                selected_color: uniforms::FACE_SELECTED_COLOR,
                is_pick_pass: uniforms::FACE_IS_PICK_PASS,
            },
            view: ViewUniforms {
                model_view: uniforms::MODEL_VIEW,
                projection: uniforms::PROJECTION,
                viewport_size: uniforms::VIEWPORT_SIZE,
            },
        },
    }
    .into_ts()?)
}
