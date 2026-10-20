//! Geometry calculations independent of mesh topology and selection.

use glam::{DMat4, DVec3};

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
