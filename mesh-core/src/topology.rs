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

    /// Incident edges in disk-cycle order. The vertex must be live.
    pub(crate) fn vert_edges(&self, vertex: VertKey) -> impl Iterator<Item = EdgeKey> + '_ {
        let first = self.verts[vertex].edge;
        let mut current = first;
        std::iter::from_fn(move || {
            let key = current?;
            let edge = &self.edges[key];
            let side = usize::from(edge.verts[1] == vertex);
            let next = edge.disk_next[side];
            current = (Some(next) != first).then_some(next);
            Some(key)
        })
    }

    pub(crate) fn vert_neighbors(&self, vertex: VertKey) -> impl Iterator<Item = VertKey> + '_ {
        self.vert_edges(vertex).map(move |key| {
            let verts = self.edge_verts(key);
            verts[usize::from(verts[0] == vertex)]
        })
    }

    pub(crate) fn vert_faces(&self, vertex: VertKey) -> impl Iterator<Item = FaceKey> + '_ {
        self.vert_loops(vertex).map(|key| self.loops[key].face)
    }

    /// Each face corner at this vertex, including disconnected fans.
    pub(crate) fn vert_loops(&self, vertex: VertKey) -> impl Iterator<Item = LoopKey> + '_ {
        self.vert_edges(vertex)
            .flat_map(|key| self.edge_loops(key))
            .filter(move |&key| self.loops[key].vert == vertex)
    }

    pub(crate) fn edge_verts(&self, edge: EdgeKey) -> [VertKey; 2] {
        self.edges[edge].verts
    }

    pub(crate) fn edge_faces(&self, edge: EdgeKey) -> impl Iterator<Item = FaceKey> + '_ {
        self.edge_loops(edge).map(|key| self.loops[key].face)
    }

    /// Radial-cycle corners, empty for a wire edge. The edge must be live.
    pub(crate) fn edge_loops(&self, edge: EdgeKey) -> impl Iterator<Item = LoopKey> + '_ {
        let first = self.edges[edge].loop_;
        let mut current = first;
        std::iter::from_fn(move || {
            let key = current?;
            let next = self.loops[key].radial_next;
            current = (Some(next) != first).then_some(next);
            Some(key)
        })
    }

    pub(crate) fn face_verts(&self, face: FaceKey) -> impl Iterator<Item = VertKey> + '_ {
        self.face_loops(face).map(|key| self.loops[key].vert)
    }

    pub(crate) fn face_edges(&self, face: FaceKey) -> impl Iterator<Item = EdgeKey> + '_ {
        self.face_loops(face).map(|key| self.loops[key].edge)
    }

    /// Corners in boundary winding order. The face must be live.
    pub(crate) fn face_loops(&self, face: FaceKey) -> impl Iterator<Item = LoopKey> + '_ {
        let face = &self.faces[face];
        let mut current = face.loop_;
        (0..face.len).map(move |_| {
            let key = current;
            current = self.loops[key].next;
            key
        })
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
