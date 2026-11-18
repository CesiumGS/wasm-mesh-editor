//! Spatial queries over a mesh's surface.

use geometry::{Aabb, closest_point_on_triangle, polygon_normal};
use glam::DVec3;
use spatial_index::SpatialIndex;

use crate::{FaceKey, Mesh, VertKey};

/// Read-only spatial queries over a mesh's faces, including hidden ones, accelerated by an index
/// over the faces' bounding boxes.
pub struct SpatialQuery<'a, I> {
    mesh: &'a Mesh,
    index: &'a I,
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

impl<'a, I: SpatialIndex<FaceKey>> SpatialQuery<'a, I> {
    /// `index` must hold the bounds of every face in `mesh`, as from [`face_bounds`].
    pub(crate) fn new(mesh: &'a Mesh, index: &'a I) -> Self {
        Self { mesh, index }
    }

    /// Returns `None` for a non-finite `point` or a mesh with no non-degenerate triangles.
    pub fn closest_point(&self, point: DVec3) -> Option<ClosestPoint> {
        if !point.is_finite() {
            return None;
        }

        let positions = self.mesh.attributes.positions();
        let mut closest: Option<(f64, ClosestPoint)> = None;

        self.index.traverse(
            |bounds| Some(bounds.distance_squared(point)),
            |face| {
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

                closest.map_or(f64::INFINITY, |(distance, _)| distance)
            },
        );

        closest.map(|(_, closest)| closest)
    }
}

/// Each face's bounding box, for building a [`SpatialIndex`] over a mesh's faces.
pub(crate) fn face_bounds(mesh: &Mesh) -> impl Iterator<Item = (Aabb, FaceKey)> + '_ {
    mesh.topology.faces().map(|face| {
        let verts = mesh.topology.face_verts(face);
        let bounds = Aabb::from_points(verts.map(|key| mesh.attributes.positions[key]));
        (bounds.unwrap(), face)
    })
}

#[cfg(test)]
#[path = "../tests/unit/spatial_query.rs"]
mod tests;
