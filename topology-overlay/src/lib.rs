//! Prepares mesh data for a topology overlay, and provides [`shaders`] that draw it with WebGL2.
//! The renderer owns GPU resources and draw calls.
//!
//! [`OverlayBuffers::component_keys`] maps buffer elements to mesh components, e.g. for picking.

pub mod shaders;
mod topologyoverlay;

pub use topologyoverlay::{OverlayBuffer, OverlayBuffers, SelectionBuffers, TopologyOverlay};
