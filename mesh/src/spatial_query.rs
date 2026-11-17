//! Spatial queries over a mesh's surface.

use glam::DVec3;

use crate::geometry::{closest_point_on_triangle, polygon_normal};
use crate::{FaceKey, Mesh, VertKey};

/// Read-only spatial queries over a mesh's faces, including hidden ones.
pub struct SpatialQuery<'a> {
    pub(crate) mesh: &'a Mesh,
}

/// The point on a mesh's surface closest to a query point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestPoint {
    pub face: FaceKey,
    /// The triangle of `face`'s [`triangulation`](crate::FaceRef::triangulation) that holds `position`.
    pub verts: [VertKey; 3],
    /// Barycentric weights of `position` for `verts`.
    pub bary: [f64; 3],
    pub position: DVec3,
}

impl SpatialQuery<'_> {
    /// Returns `None` for a non-finite `point` or a mesh with no non-degenerate triangles.
    pub fn closest_point(&self, point: DVec3) -> Option<ClosestPoint> {
        if !point.is_finite() {
            return None;
        }

        let positions = self.mesh.attributes.positions();
        let mut closest: Option<(f64, ClosestPoint)> = None;

        for face in self.mesh.topology.faces() {
            for verts in self.mesh.face(face).unwrap().triangulation() {
                let triangle = verts.map(|key| positions[key].as_dvec3());
                if polygon_normal(triangle).is_none() {
                    continue;
                }

                let bary = closest_point_on_triangle(point, triangle);
                let position =
                    triangle[0] * bary[0] + triangle[1] * bary[1] + triangle[2] * bary[2];
                let distance = point.distance_squared(position);

                if closest.is_none_or(|(closest_distance, _)| distance < closest_distance) {
                    closest = Some((
                        distance,
                        ClosestPoint {
                            face,
                            verts,
                            bary,
                            position,
                        },
                    ));
                }
            }
        }

        closest.map(|(_, closest)| closest)
    }
}

#[cfg(test)]
#[path = "../tests/unit/spatial_query.rs"]
mod tests;
