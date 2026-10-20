//! The boundary of a selection.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use super::SelectionState;
use crate::{FaceKey, LoopKey, Topology, VertKey};

/// How grow, shrink, and boundary queries determine what a neighbor is
/// (i.e. whether neighbors are connected by an edge or share a face)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionStep {
    /// Vertices or faces sharing an edge.
    Edge,
    /// Vertices sharing a face, or faces sharing a vertex.
    Face,
}

#[derive(Default)]
pub(super) struct BoundarySets<K> {
    pub(super) inner: OnceLock<HashSet<K>>,
    pub(super) outer: OnceLock<HashSet<K>>,
}

impl<K> BoundarySets<K> {
    fn side(&self, selected: bool) -> &OnceLock<HashSet<K>> {
        if selected { &self.inner } else { &self.outer }
    }
}

/// Lazy sets shared across boundary views. Position edits leave them cached.
#[derive(Default)]
pub(super) struct BoundaryCache {
    pub(super) edge_step_vertices: BoundarySets<VertKey>,
    pub(super) face_step_vertices: BoundarySets<VertKey>,
    pub(super) edge_step_faces: BoundarySets<FaceKey>,
    pub(super) face_step_faces: BoundarySets<FaceKey>,
    pub(super) mixed_faces: OnceLock<HashSet<FaceKey>>,
    pub(super) mixed_face_loops: OnceLock<Rc<Vec<LoopKey>>>,
}

/// The selected (inner) and unselected (outer) sides of a selection boundary.
///
/// Each set is lazily computed, and reused until the selection or topology changes.
pub struct SelectionBoundary<'a> {
    selection: &'a SelectionState,
    topo: &'a Topology,
    cache: &'a BoundaryCache,
}

impl<'a> SelectionBoundary<'a> {
    pub(crate) fn new(selection: &'a SelectionState, topo: &'a Topology) -> Self {
        Self {
            selection,
            topo,
            cache: &selection.boundary,
        }
    }

    /// Selected vertices adjacent to unselected vertices.
    pub fn inner_vertices(&self, step: SelectionStep) -> impl Iterator<Item = VertKey> + '_ {
        self.vertex_boundary(step, true).iter().copied()
    }

    /// Unselected vertices adjacent to selected vertices.
    pub fn outer_vertices(&self, step: SelectionStep) -> impl Iterator<Item = VertKey> + '_ {
        self.vertex_boundary(step, false).iter().copied()
    }

    /// Selected faces adjacent to unselected faces.
    pub fn inner_faces(&self, step: SelectionStep) -> impl Iterator<Item = FaceKey> + '_ {
        self.face_boundary(step, true).iter().copied()
    }

    /// Unselected faces adjacent to selected faces.
    pub fn outer_faces(&self, step: SelectionStep) -> impl Iterator<Item = FaceKey> + '_ {
        self.face_boundary(step, false).iter().copied()
    }

    /// Faces with both selected and unselected vertices.
    /// In addition to being useful intermediate state, this also identifies the set of faces whose normals
    /// need updating in flat-shading mode (thus, it's public to the crate, but not externally).
    pub(crate) fn mixed_faces(&self) -> &HashSet<FaceKey> {
        self.cache.mixed_faces.get_or_init(|| {
            let mut mixed = HashSet::new();
            let mut visited_faces = HashSet::new();

            for &vert_key in self.selection.verts.iter() {
                for face_key in self.topo.vert_faces(vert_key) {
                    if !visited_faces.insert(face_key) {
                        continue;
                    }

                    if self
                        .topo
                        .face_verts(face_key)
                        .all(|key| self.selection.verts.contains(&key))
                    {
                        continue;
                    }

                    mixed.insert(face_key);
                }
            }

            mixed
        })
    }

    /// Shared corner keys for every mixed face, including corners at unselected vertices.
    pub(crate) fn mixed_face_loops(&self) -> &Rc<Vec<LoopKey>> {
        self.cache.mixed_face_loops.get_or_init(|| {
            Rc::new(
                self.mixed_faces()
                    .iter()
                    .flat_map(|&key| self.topo.face_loops(key))
                    .collect(),
            )
        })
    }

    fn vertex_boundary(&self, step: SelectionStep, selected: bool) -> &HashSet<VertKey> {
        let cache = match step {
            SelectionStep::Edge => &self.cache.edge_step_vertices,
            SelectionStep::Face => &self.cache.face_step_vertices,
        };

        cache.side(selected).get_or_init(|| {
            let mut boundary = HashSet::new();

            for &key in self.selection.verts.iter() {
                for edge_key in self.topo.vert_edges(key) {
                    let edge = &self.topo.edges[edge_key];
                    // When stepping by face, skip edges that are part of a face (i.e., not wire edges)
                    if step == SelectionStep::Face && edge.loop_.is_some() {
                        continue;
                    }

                    // If neighbor is selected, this vertex is not on the boundary
                    let neighbor = edge.verts[usize::from(edge.verts[0] == key)];
                    if self.selection.verts.contains(&neighbor) {
                        continue;
                    }

                    boundary.insert(if selected { key } else { neighbor });
                }
            }

            if step == SelectionStep::Edge {
                return boundary;
            }

            for &face_key in self.mixed_faces() {
                boundary.extend(
                    self.topo
                        .face_verts(face_key)
                        .filter(|key| self.selection.verts.contains(key) == selected),
                );
            }

            boundary
        })
    }

    fn face_boundary(&self, step: SelectionStep, selected: bool) -> &HashSet<FaceKey> {
        let cache = match step {
            SelectionStep::Edge => &self.cache.edge_step_faces,
            SelectionStep::Face => &self.cache.face_step_faces,
        };

        cache.side(selected).get_or_init(|| {
            let mut boundary = HashSet::new();
            for &key in &self.selection.faces {
                let mut visit = |neighbor| {
                    if self.selection.faces.contains(&neighbor) {
                        return;
                    }

                    boundary.insert(if selected { key } else { neighbor });
                };

                if step == SelectionStep::Edge {
                    for edge_key in self.topo.face_edges(key) {
                        for neighbor in self.topo.edge_faces(edge_key) {
                            visit(neighbor);
                        }
                    }
                    continue;
                }

                for vert_key in self.topo.face_verts(key) {
                    for neighbor in self.topo.vert_faces(vert_key) {
                        visit(neighbor);
                    }
                }
            }
            boundary
        })
    }
}
