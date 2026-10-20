//! GLSL ES 3.00 shaders that draw [`OverlayBuffers`](crate::OverlayBuffers) as WebGL2 data
//! textures, one instanced draw per component kind.
//!
//! Each program owns its `main()` and calls hook functions that the consumer defines, keeping
//! camera transforms, depth writing, and pick encoding on the consumer's side. Build a stage as
//! `#version 300 es`, then the program's source, then the consumer's hook definitions:
//! [`DEFAULT_HOOKS`] for plain WebGL2, or the consumer's own.

/// The vertex and fragment sources of one program.
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

/// Names of the texture uniforms that the programs read overlay buffers from.
pub mod uniforms {
    /// `positions`, in every program.
    pub const POSITION_TEXTURE: &str = "u_positionTexture";
    /// The drawn component kind's selection buffer, in every program: `selection.verts` for
    /// points, `selection.edges` for edges, and `selection.faces` for faces.
    pub const SELECTION_TEXTURE: &str = "u_selectionTexture";
    /// `edges`, in the edge program.
    pub const EDGE_TEXTURE: &str = "u_edgeTexture";
    /// `triangles`, in the face program.
    pub const TRIANGLE_TEXTURE: &str = "u_triangleTexture";
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
