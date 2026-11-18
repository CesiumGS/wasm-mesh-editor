use glam::{DMat4, DVec3, Vec3};
use mesh::{Mesh, VertKey};

/// Points bound to a deformer mesh's triangles, so they follow the triangles as the mesh is edited.
pub struct SurfaceBinding {
    /// One per deformable position, in input order, expressed in the frame of its closest deformer
    /// triangle.
    pub points: Vec<BoundPoint>,
    /// The largest distance from a point to the deformer's surface, in deformer-local space.
    pub max_distance: f64,
}

/// A point in the frame of its deformer triangle `verts = [P0, P1, P2]`:
/// `P0 + a * (P1 - P0) + b * (P2 - P0) + c * n`, where `[a, b, c] = coords` and `n` is the
/// triangle's unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundPoint {
    pub verts: [VertKey; 3],
    pub coords: DVec3,
}

impl SurfaceBinding {
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
        let mut max_distance_squared = 0.0_f64;

        let points = deformable_positions
            .iter()
            .map(|position| {
                let point = deformable_to_deformer.transform_point3(position.as_dvec3());
                let closest_deformer_point = deformer_query.closest_point(point)?;
                max_distance_squared = max_distance_squared
                    .max(point.distance_squared(closest_deformer_point.position));

                let triangle = closest_deformer_point
                    .verts
                    .map(|key| positions[key].as_dvec3());

                Some(BoundPoint {
                    verts: closest_deformer_point.verts,
                    coords: frame_coords(point, triangle),
                })
            })
            .collect::<Option<_>>()?;

        Some(Self {
            points,
            max_distance: max_distance_squared.sqrt(),
        })
    }

    /// Packs the binding for a GPU vertex buffer, as `[i0, i1, i2, a, b, c]` per point in
    /// `points` order. `vertex_index` maps each triangle vertex to its index in the buffer of
    /// deformer positions that the shader reads `P0`, `P1` and `P2` from, such as
    /// `TopologyOverlay::vertex_buffer_index`.
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
            let [i0, i1, i2] = point.verts.map(|key| vertex_index(key) as f32);
            let [a, b, c] = point.coords.as_vec3().to_array();
            out.copy_from_slice(&[i0, i1, i2, a, b, c]);
        }
    }
}

/// `[a, b, c]` such that `point = p0 + a * (p1 - p0) + b * (p2 - p0) + c * n`, where `n` is the
/// triangle's unit normal. The triangle must not be degenerate.
fn frame_coords(point: DVec3, [p0, p1, p2]: [DVec3; 3]) -> DVec3 {
    let (e1, e2, offset) = (p1 - p0, p2 - p0, point - p0);
    let normal = e1.cross(e2);
    let length_squared = normal.length_squared();

    DVec3::new(
        offset.cross(e2).dot(normal) / length_squared,
        e1.cross(offset).dot(normal) / length_squared,
        offset.dot(normal) / length_squared.sqrt(),
    )
}

#[cfg(test)]
#[path = "../tests/unit/surface_binding.rs"]
mod tests;
