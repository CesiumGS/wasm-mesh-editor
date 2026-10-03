//! Geometry calculations independent of mesh topology and selection.

use glam::DVec3;

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
