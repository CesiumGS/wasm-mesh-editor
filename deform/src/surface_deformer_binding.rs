use geometry::Triangle;
use glam::{DMat4, DVec3, Vec3};
use mesh::{Mesh, VertKey};

/// Points bound to a deformer mesh's triangles, so they follow the triangles as the mesh is edited.
///
/// As the deformer's vertices move, every deformed point stays within the deformer's bounds
/// expanded on each axis by `max_overshoot * extent + max_normal_distance`, where `extent` is the
/// deformer's bounds' size on that axis.
pub struct SurfaceDeformerBinding {
    /// One per deformable position, in input order, expressed in the frame of its closest deformer
    /// triangle.
    pub points: Vec<BoundPoint>,
    /// The largest distance of a point off its triangle's plane: the largest `|c|`.
    pub max_normal_distance: f64,
    /// The largest total of a point's negative barycentric weights `(1 - a - b, a, b)`: how far
    /// outside its triangle a point projects, relative to the triangle's size. Zero if every point
    /// projects inside its triangle.
    pub max_overshoot: f64,
}

/// A point in the frame of its deformer triangle `deformer_verts = [P0, P1, P2]`:
/// `P0 + a * (P1 - P0) + b * (P2 - P0) + c * n`, where `[a, b, c] = frame_coords` and `n` is the
/// triangle's unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundPoint {
    pub deformer_verts: [VertKey; 3],
    pub frame_coords: DVec3,
}

impl SurfaceDeformerBinding {
    /// Binds each of `deformable_positions` to its closest triangle of `deformer`, after
    /// transforming it into deformer-local space. Returns `None` if a point has no closest point
    /// (see [`SpatialQuery::closest_point`](mesh::SpatialQuery::closest_point)).
    pub fn new(
        deformer: &Mesh,
        deformable_positions: &[Vec3],
        deformable_to_deformer: DMat4,
    ) -> Option<Self> {
        let deformer_query = deformer.spatial_query();
        let positions = &deformer.attributes().positions;
        let mut max_normal_distance = 0.0_f64;
        let mut max_overshoot = 0.0_f64;

        let points = deformable_positions
            .iter()
            .map(|position| {
                let point = deformable_to_deformer.transform_point3(position.as_dvec3());
                let deformer_verts = deformer_query.closest_point(point)?.verts;
                let triangle = Triangle::new(deformer_verts.map(|key| positions[key].as_dvec3()))?;
                let frame_coords = triangle.to_frame(point);

                // Compute the overshoot of the point relative to its triangle
                // and compare it to the running maximums.
                let [a, b, c] = frame_coords.to_array();
                let overshoot: f64 = [1.0 - a - b, a, b]
                    .map(|weight| (-weight).max(0.0))
                    .iter()
                    .sum();

                max_normal_distance = max_normal_distance.max(c.abs());
                max_overshoot = max_overshoot.max(overshoot);

                Some(BoundPoint {
                    deformer_verts,
                    frame_coords,
                })
            })
            .collect::<Option<_>>()?;

        Some(Self {
            points,
            max_normal_distance,
            max_overshoot,
        })
    }

    /// Packs the binding for a GPU vertex buffer, as `[i0, i1, i2, a, b, c]` per point in
    /// `points` order. `vertex_index` maps each triangle vertex to its index in the buffer of
    /// deformer positions that the shader reads `P0`, `P1` and `P2` from.
    ///
    /// # Panics
    /// If `out` doesn't hold exactly 6 values per point.
    pub fn pack(&self, vertex_index: impl Fn(VertKey) -> u32, out: &mut [f32]) {
        assert_eq!(
            out.len(),
            6 * self.points.len(),
            "out must hold 6 values per point"
        );

        for (point, out) in self.points.iter().zip(out.chunks_exact_mut(6)) {
            let [i0, i1, i2] = point.deformer_verts.map(|key| vertex_index(key) as f32);
            let [a, b, c] = point.frame_coords.as_vec3().to_array();
            out.copy_from_slice(&[i0, i1, i2, a, b, c]);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/surface_deformer_binding.rs"]
mod tests;
