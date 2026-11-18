//! Geometry types and calculations, independent of any mesh data structure.

mod geometry;
mod triangle;

pub use geometry::{Aabb, Plane, Ray, centroid, corner_angle, polygon_normal, try_inverse_affine};
pub use triangle::Triangle;
