//! Prepares mesh data for a topology overlay. Rendering is handled elsewhere.
//!
//! Picks read back from the overlay buffers map to mesh components through
//! [`TopologyOverlay::component_key`].

mod topologyoverlay;

pub use topologyoverlay::{
    OverlayBuffer, OverlayBuffers, OverlayComponent, SelectionBuffers, TopologyOverlay,
};
