//! Acceleration structures for finding items by their bounding boxes.

mod bvh;
mod spatial_index;

pub use bvh::Bvh;
pub use spatial_index::SpatialIndex;
