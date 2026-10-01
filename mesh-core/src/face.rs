//! Face component and its composed view.

use glam::{DVec3, Vec3};

use crate::{Attributes, EdgeKey, FaceKey, LoopKey, Topology, VertKey};

/// A face (n-gon), reached from an entry loop.
pub struct Face {
    pub loop_: LoopKey,
    pub len: u32,
}

/// A face view composing connectivity with geometry.
pub struct FaceRef<'a> {
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
    pub(crate) key: FaceKey,
}

impl<'a> FaceRef<'a> {
    pub fn key(&self) -> FaceKey {
        self.key
    }

    /// The face's vertices in boundary winding order.
    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.loops().map(|key| self.topo.loops[key].vert)
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        let face = &self.topo.faces[self.key];
        let mut current = face.loop_;
        (0..face.len).map(move |_| {
            let key = current;
            current = self.topo.loops[key].next;
            key
        })
    }

    pub fn triangulate(&self) -> impl Iterator<Item = [VertKey; 3]> + '_ {
        std::iter::empty::<[VertKey; 3]>()
    }

    /// The face's geometric normal (distinct from the per-loop shading normals).
    /// Computed from current positions without changing stored shading normals.
    /// Returns zero for a zero-area face.
    pub fn normal(&self) -> Vec3 {
        let mut positions = self.verts().map(|key| self.attrs.positions[key].as_dvec3());

        let Some(origin) = positions.next() else {
            return Vec3::ZERO;
        };
        let Some(second) = positions.next() else {
            return Vec3::ZERO;
        };

        let mut previous = second - origin;
        let mut normal = DVec3::ZERO;

        for position in positions {
            let current = position - origin;
            normal += previous.cross(current);
            previous = current;
        }

        normal.normalize_or_zero().as_vec3()
    }
}
