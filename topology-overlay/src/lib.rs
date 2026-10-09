//! Prepares mesh data for a topology overlay. Rendering is handled elsewhere.
//!
//! API draft: picking lookup is not implemented yet.

mod topologyoverlay;

pub use topologyoverlay::{OverlayBuffer, OverlayBuffers, SelectionBuffers, TopologyOverlay};
