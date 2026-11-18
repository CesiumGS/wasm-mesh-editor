//! Geometry types and calculations, independent of any mesh data structure.

mod geometry;

pub use geometry::{
    Aabb, Plane, Ray, centroid, closest_point_on_triangle, corner_angle, polygon_normal,
    try_inverse_affine,
};
