//! Prepares mesh data for a topology overlay. Rendering is handled elsewhere.
//!
//! API draft: incremental updates and picking lookup are not implemented yet.

mod topologyoverlay;

pub use topologyoverlay::{OverlayBuffer, OverlayBuffers, TopologyOverlay};
