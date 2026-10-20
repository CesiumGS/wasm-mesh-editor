//! GLSL ES 3.00 shaders that draw [`OverlayBuffers`](crate::OverlayBuffers) as WebGL2 data
//! textures, one instanced draw per component kind.
//!
//! Each shader owns its `main()` and calls hook functions that the consumer defines, keeping
//! camera transforms, depth writing, and pick encoding on the consumer's side. Build a stage as
//! `#version 300 es`, then the shader's source, then the consumer's hook definitions:
//! [`DEFAULT_HOOKS`] for plain WebGL2, or the consumer's own.

/// The vertex and fragment sources of one shader.
#[derive(Clone, Copy, Debug)]
pub struct ShaderSources {
    pub vertex: &'static str,
    pub fragment: &'static str,
}

macro_rules! shader {
    ($name:literal) => {
        include_str!(concat!("../shaders/", $name, ".glsl"))
    };
}

/// Points: draw `positions.len()` instances of 4 vertices each as `TRIANGLE_STRIP`.
/// `u_pointSize` is a diameter in the pixels of `topologyOverlay_viewportSize()`.
pub const POINT: ShaderSources = ShaderSources {
    vertex: concat!(
        shader!("TopologyOverlayCommonVS"),
        shader!("TopologyOverlayPointVS")
    ),
    fragment: concat!(
        shader!("TopologyOverlayCommonFS"),
        shader!("TopologyOverlayPointFS")
    ),
};

/// Edges: draw `edges.len()` instances of 4 vertices each as `TRIANGLE_STRIP`.
/// `u_edgeWidth` is in the pixels of `topologyOverlay_viewportSize()`.
pub const EDGE: ShaderSources = ShaderSources {
    vertex: concat!(
        shader!("TopologyOverlayCommonVS"),
        shader!("TopologyOverlayEdgeVS")
    ),
    fragment: concat!(
        shader!("TopologyOverlayCommonFS"),
        shader!("TopologyOverlayEdgeFS")
    ),
};

/// Faces: draw `triangles.len()` instances of 3 vertices each as `TRIANGLES`.
pub const FACE: ShaderSources = ShaderSources {
    vertex: concat!(
        shader!("TopologyOverlayCommonVS"),
        shader!("TopologyOverlayFaceVS")
    ),
    fragment: concat!(
        shader!("TopologyOverlayCommonFS"),
        shader!("TopologyOverlayFaceFS")
    ),
};

/// Names of the shaders' uniforms. Each shader declares exactly the ones its docs list.
pub mod uniforms {
    // ---- Overlay buffer textures ----

    /// `positions`, in every shader.
    pub const POSITION_TEXTURE: &str = "u_positionTexture";
    /// The drawn component kind's selection buffer, in every shader: `selection.verts` for
    /// points, `selection.edges` for edges, and `selection.faces` for faces.
    pub const SELECTION_TEXTURE: &str = "u_selectionTexture";
    /// `edges`, in the edge shader.
    pub const EDGE_TEXTURE: &str = "u_edgeTexture";
    /// `triangles`, in the face shader.
    pub const TRIANGLE_TEXTURE: &str = "u_triangleTexture";

    // ---- Style ----

    /// `float`, in every shader: pulls the overlay toward the camera by this fraction of its
    /// eye-space depth. Set per draw, e.g. larger for points than edges, and edges than faces,
    /// so points draw over edges and edges over faces.
    pub const DEPTH_BIAS: &str = "u_depthBias";
    /// `float`, in the point shader: diameter in pixels.
    pub const POINT_SIZE: &str = "u_pointSize";
    /// `vec4`, in the point shader: color of unselected points.
    pub const POINT_COLOR: &str = "u_pointColor";
    /// `vec4`, in the point shader: color of selected points.
    pub const POINT_SELECTED_COLOR: &str = "u_pointSelectedColor";
    /// `float`, in the edge shader: width in pixels.
    pub const EDGE_WIDTH: &str = "u_edgeWidth";
    /// `vec4`, in the edge shader: color of unselected edges.
    pub const EDGE_COLOR: &str = "u_edgeColor";
    /// `vec4`, in the edge shader: color of selected edges.
    pub const EDGE_SELECTED_COLOR: &str = "u_edgeSelectedColor";
    /// `vec4`, in the face shader: color of unselected faces. Often transparent.
    pub const FACE_COLOR: &str = "u_faceColor";
    /// `vec4`, in the face shader: color of selected faces.
    pub const FACE_SELECTED_COLOR: &str = "u_faceSelectedColor";
    /// `bool`, in the face shader: draws every face opaque and unshaded, so transparent faces
    /// stay pickable in a pick pass.
    pub const FACE_IS_PICK_PASS: &str = "u_isPickPass";

    // ---- View, declared by the default hooks ----

    /// `mat4`, in the default vertex hooks: model to eye space.
    pub const MODEL_VIEW: &str = "u_modelView";
    /// `mat4`, in the default vertex hooks: eye to clip space.
    pub const PROJECTION: &str = "u_projection";
    /// `vec2`, in the default vertex hooks: viewport size in pixels.
    pub const VIEWPORT_SIZE: &str = "u_viewportSize";
}

/// Hook definitions for plain WebGL2 consumers, driven by `u_modelView`, `u_projection`, and
/// `u_viewportSize` uniforms. They don't encode picks.
pub const DEFAULT_HOOKS: ShaderSources = ShaderSources {
    vertex: shader!("TopologyOverlayDefaultHooksVS"),
    fragment: shader!("TopologyOverlayDefaultHooksFS"),
};

#[cfg(test)]
#[path = "../tests/unit/shaders.rs"]
mod tests;
