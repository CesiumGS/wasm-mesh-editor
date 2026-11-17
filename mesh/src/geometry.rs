//! Geometry calculations independent of mesh topology and selection.

use glam::{DMat4, DVec3, Vec3};

/// A ray with a finite origin and unit-length direction, in any coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: DVec3,
    pub direction: DVec3,
}

/// An infinite plane with a finite position and unit-length normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    position: DVec3,
    normal: DVec3,
}

impl Plane {
    /// Returns `None` for a non-finite position or a non-unit normal.
    pub fn new(position: DVec3, normal: DVec3) -> Option<Self> {
        if !position.is_finite() || !normal.is_finite() || !normal.is_normalized() {
            return None;
        }

        Some(Self { position, normal })
    }

    /// Returns the forward ray intersection, or `None` for invalid rays,
    /// near-parallel rays, intersections behind the origin, or non-finite results.
    pub fn intersect(&self, ray: Ray) -> Option<DVec3> {
        if !ray.origin.is_finite() || !ray.direction.is_finite() || !ray.direction.is_normalized() {
            return None;
        }

        let denominator = self.normal.dot(ray.direction);

        if denominator.abs() <= 1e-12 {
            return None;
        }

        let distance = (self.position - ray.origin).dot(self.normal) / denominator;

        if !distance.is_finite() || distance < 0.0 {
            return None;
        }

        let hit = ray.origin + distance * ray.direction;

        hit.is_finite().then_some(hit)
    }
}

/// An axis-aligned bounding box with `min <= max` on every axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Returns `None` for no points.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let initial = Self {
            min: first,
            max: first,
        };

        Some(points.fold(initial, |aabb, point| Self {
            min: aabb.min.min(point),
            max: aabb.max.max(point),
        }))
    }

    /// The smallest box containing both boxes.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Translate the bounding box by the given vector.
    /// In the future: this should be a full transform method.
    #[must_use]
    pub fn translated(self, delta: Vec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }
}

/// Returns a finite inverse, or `None` for non-affine, singular, or non-finite matrices.
pub fn try_inverse_affine(matrix: DMat4) -> Option<DMat4> {
    if !matrix.is_finite()
        || matrix.x_axis.w != 0.0
        || matrix.y_axis.w != 0.0
        || matrix.z_axis.w != 0.0
        || matrix.w_axis.w != 1.0
    {
        return None;
    }

    let determinant = matrix.determinant();

    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }

    let inverse = matrix.inverse();

    inverse.is_finite().then_some(inverse)
}

/// Returns the mean position and point count, or `None` for non-finite input or sums.
/// Empty input returns `(DVec3::ZERO, 0)`.
pub fn centroid(positions: impl IntoIterator<Item = DVec3>) -> Option<(DVec3, usize)> {
    let mut position_sum = DVec3::ZERO;
    let mut point_count = 0usize;

    for position in positions {
        if !position.is_finite() {
            return None;
        }

        position_sum += position;
        point_count += 1;
    }

    if point_count == 0 {
        return Some((DVec3::ZERO, 0));
    }

    let center = position_sum / point_count as f64;

    center.is_finite().then_some((center, point_count))
}

/// For planar polygons, points along the normal and has length equal to the area.
pub(crate) fn polygon_area_vector(positions: impl IntoIterator<Item = DVec3>) -> DVec3 {
    let mut positions = positions.into_iter();
    let Some(origin) = positions.next() else {
        return DVec3::ZERO;
    };
    let Some(second) = positions.next() else {
        return DVec3::ZERO;
    };

    let mut previous = second - origin;
    let mut area = DVec3::ZERO;
    for position in positions {
        let current = position - origin;
        area += previous.cross(current);
        previous = current;
    }
    area * 0.5
}

/// A unit normal, or `None` when the polygon has no usable area.
pub(crate) fn polygon_normal(positions: impl IntoIterator<Item = DVec3>) -> Option<DVec3> {
    polygon_area_vector(positions).try_normalize()
}

/// Finds the point on the triangle `[a, b, c]` closest to `point`, returned as barycentric weights
/// `[wa, wb, wc]` such that the closest point is `wa * a + wb * b + wc * c`. All positions must be
/// in the same space. From Ericson's Real-Time Collision Detection (5.1.5).
/// The input triangle must not be degenerate.
pub(crate) fn closest_point_on_triangle(point: DVec3, [a, b, c]: [DVec3; 3]) -> [f64; 3] {
    let ab = b - a;
    let ac = c - a;

    let ap = point - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }

    let bp = point - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }

    let cp = point - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }

    let denominator = va + vb + vc;
    let v = vb / denominator;
    let w = vc / denominator;
    [1.0 - v - w, v, w]
}

/// The angle inside the face, including corners greater than 180 degrees.
pub(crate) fn corner_angle(to_previous: DVec3, to_next: DVec3, face_normal: DVec3) -> f64 {
    let Some(previous) = to_previous.try_normalize() else {
        return 0.0;
    };
    let Some(next) = to_next.try_normalize() else {
        return 0.0;
    };

    let cross = previous.cross(next);
    let angle = cross.length().atan2(previous.dot(next));
    if cross.dot(face_normal) > 0.0 {
        return std::f64::consts::TAU - angle;
    }
    angle
}

#[cfg(test)]
#[path = "../tests/unit/geometry.rs"]
mod tests;
