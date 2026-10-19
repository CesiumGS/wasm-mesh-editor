//! Prepares mesh data for a topology overlay, and provides [`shaders`] that draw it with WebGL2.
//! The renderer owns GPU resources and draw calls.
//!
//! Picks read back from the overlay buffers map to mesh components through
//! [`TopologyOverlay::component_key`].

pub mod shaders;
mod topologyoverlay;

pub use topologyoverlay::{
    OverlayBuffer, OverlayBuffers, OverlayComponent, SelectionBuffers, TopologyOverlay,
};
