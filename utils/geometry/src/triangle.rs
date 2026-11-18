use glam::DVec3;

/// A triangle with nonzero area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle([DVec3; 3]);

impl Triangle {
    /// Returns `None` for a triangle with no usable area.
    pub fn new(positions: [DVec3; 3]) -> Option<Self> {
        let [p0, p1, p2] = positions;
        let normal = (p1 - p0).cross(p2 - p0).try_normalize();
        normal.map(|_| Self(positions))
    }

    pub fn positions(&self) -> [DVec3; 3] {
        self.0
    }

    /// The barycentric weights of the point on the triangle closest to `point`. From Ericson's
    /// Real-Time Collision Detection (5.1.5).
    pub fn closest_point(&self, point: DVec3) -> [f64; 3] {
        let [a, b, c] = self.0;
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

    /// The point with barycentric weights `bary`.
    pub fn point_at(&self, bary: [f64; 3]) -> DVec3 {
        let [p0, p1, p2] = self.0;
        p0 * bary[0] + p1 * bary[1] + p2 * bary[2]
    }

    /// `[a, b, c]` such that `point = P0 + a * (P1 - P0) + b * (P2 - P0) + c * n`, where
    /// `[P0, P1, P2]` are the positions and `n` is the unit normal.
    pub fn to_frame(&self, point: DVec3) -> DVec3 {
        let [p0, p1, p2] = self.0;
        let (e1, e2, offset) = (p1 - p0, p2 - p0, point - p0);
        let normal = e1.cross(e2);
        let length_squared = normal.length_squared();

        DVec3::new(
            offset.cross(e2).dot(normal) / length_squared,
            e1.cross(offset).dot(normal) / length_squared,
            offset.dot(normal) / length_squared.sqrt(),
        )
    }
}

#[cfg(test)]
#[path = "../tests/unit/triangle.rs"]
mod tests;
