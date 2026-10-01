//! The connectivity graph: the four component `SlotMap`s.

use slotmap::{Key, SlotMap};

use crate::{ComponentKey, Edge, EdgeKey, Face, FaceKey, Loop, LoopKey, Vert, VertKey};

/// Stable-handle storage for the mesh's verts, edges, loops, and faces.
#[derive(Default)]
pub struct Topology {
    pub(crate) verts: SlotMap<VertKey, Vert>,
    pub(crate) edges: SlotMap<EdgeKey, Edge>,
    pub(crate) loops: SlotMap<LoopKey, Loop>,
    pub(crate) faces: SlotMap<FaceKey, Face>,
}

impl Topology {
    /// Whether this topology contains the given vertex, edge, or face handle.
    /// Missing and stale handles return false.
    pub fn contains<K: Into<ComponentKey>>(&self, key: K) -> bool {
        match key.into() {
            ComponentKey::Vert(key) => self.verts.contains_key(key),
            ComponentKey::Edge(key) => self.edges.contains_key(key),
            ComponentKey::Face(key) => self.faces.contains_key(key),
        }
    }

    /// Creates empty topology with space for vertices and loops.
    /// Edges and faces grow as needed.
    pub fn with_capacity(vertex_capacity: usize, loop_capacity: usize) -> Self {
        Self {
            verts: SlotMap::with_capacity_and_key(vertex_capacity),
            edges: SlotMap::with_key(),
            loops: SlotMap::with_capacity_and_key(loop_capacity),
            faces: SlotMap::with_key(),
        }
    }

    pub fn vert_count(&self) -> usize {
        todo!()
    }

    pub fn edge_count(&self) -> usize {
        todo!()
    }

    pub fn loop_count(&self) -> usize {
        todo!()
    }

    pub fn face_count(&self) -> usize {
        todo!()
    }

    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    pub fn loops(&self) -> impl Iterator<Item = LoopKey> + '_ {
        std::iter::empty::<LoopKey>()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }

    pub(crate) fn insert_edge(&mut self, verts: [VertKey; 2]) -> EdgeKey {
        let edge = self.edges.insert_with_key(|key| Edge {
            verts,
            disk_next: [key; 2],
            disk_prev: [key; 2],
            loop_: None,
        });

        for (side, vert) in verts.into_iter().enumerate() {
            let Some(first) = self.verts[vert].edge else {
                self.verts[vert].edge = Some(edge);
                continue;
            };
            let first_side = usize::from(self.edges[first].verts[1] == vert);
            let last = self.edges[first].disk_prev[first_side];
            let last_side = usize::from(self.edges[last].verts[1] == vert);
            self.edges[edge].disk_next[side] = first;
            self.edges[edge].disk_prev[side] = last;
            self.edges[last].disk_next[last_side] = edge;
            self.edges[first].disk_prev[first_side] = edge;
        }

        edge
    }

    /// Builds a face from boundary-ordered (vertex, outgoing edge) pairs.
    /// Requires at least three distinct vertices and existing edges connecting
    /// each vertex to the next, including the closing edge.
    pub(crate) fn insert_face(&mut self, corners: &[(VertKey, EdgeKey)]) -> FaceKey {
        let face = self.faces.insert(Face {
            loop_: LoopKey::null(),
            len: corners.len() as u32,
        });

        for &(vert, edge) in corners {
            self.insert_loop(vert, edge, face);
        }

        face
    }

    fn insert_loop(&mut self, vert: VertKey, edge: EdgeKey, face: FaceKey) -> LoopKey {
        let loop_key = self.loops.insert_with_key(|key| Loop {
            vert,
            edge,
            face,
            next: key,
            prev: key,
            radial_next: key,
            radial_prev: key,
        });

        let first = self.faces[face].loop_;
        if first.is_null() {
            self.faces[face].loop_ = loop_key;
        } else {
            let last = self.loops[first].prev;
            self.loops[loop_key].next = first;
            self.loops[loop_key].prev = last;
            self.loops[last].next = loop_key;
            self.loops[first].prev = loop_key;
        }

        let Some(first) = self.edges[edge].loop_ else {
            self.edges[edge].loop_ = Some(loop_key);
            return loop_key;
        };
        let last = self.loops[first].radial_prev;
        self.loops[loop_key].radial_next = first;
        self.loops[loop_key].radial_prev = last;
        self.loops[last].radial_next = loop_key;
        self.loops[first].radial_prev = loop_key;
        loop_key
    }
}
