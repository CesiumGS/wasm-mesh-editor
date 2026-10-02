//! The boundary of a selection.

use std::collections::HashSet;
use std::sync::OnceLock;

use super::SelectionView;
use crate::{EdgeRef, FaceKey, FaceRef, VertKey, VertRef};

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
}

/// The selected (inner) and unselected (outer) sides of a selection boundary.
///
/// Each set is lazily computed, and reused until the selection or topology changes.
pub struct SelectionBoundary<'a> {
    selection: SelectionView<'a>,
    cache: &'a BoundaryCache,
}

impl<'a> SelectionBoundary<'a> {
    pub(super) fn new(selection: &SelectionView<'a>) -> Self {
        Self {
            selection: SelectionView {
                state: selection.state,
                topo: selection.topo,
                attrs: selection.attrs,
            },
            cache: &selection.state.boundary,
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

            for vert_key in self.selection.verts() {
                for face_key in self.vertex(vert_key).faces() {
                    if !visited_faces.insert(face_key) {
                        continue;
                    }

                    if self
                        .face(face_key)
                        .verts()
                        .all(|key| self.selection.contains(key))
                    {
                        continue;
                    }

                    mixed.insert(face_key);
                }
            }

            mixed
        })
    }

    fn vertex_boundary(&self, step: SelectionStep, selected: bool) -> &HashSet<VertKey> {
        let cache = match step {
            SelectionStep::Edge => &self.cache.edge_step_vertices,
            SelectionStep::Face => &self.cache.face_step_vertices,
        };

        cache.side(selected).get_or_init(|| {
            let mut boundary = HashSet::new();

            for key in self.selection.verts() {
                for edge_key in self.vertex(key).edges() {
                    let edge = &self.selection.topo.edges[edge_key];
                    // When stepping by face, skip edges that are part of a face (i.e., not wire edges)
                    if step == SelectionStep::Face && edge.loop_.is_some() {
                        continue;
                    }

                    // If neighbor is selected, this vertex is not on the boundary
                    let neighbor = edge.verts[usize::from(edge.verts[0] == key)];
                    if self.selection.contains(neighbor) {
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
                    self.face(face_key)
                        .verts()
                        .filter(|&key| self.selection.contains(key) == selected),
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
            for key in self.selection.faces() {
                let face = self.face(key);
                let mut visit = |neighbor| {
                    if self.selection.contains(neighbor) {
                        return;
                    }

                    boundary.insert(if selected { key } else { neighbor });
                };

                if step == SelectionStep::Edge {
                    for edge_key in face.edges() {
                        let edge = EdgeRef {
                            topo: self.selection.topo,
                            key: edge_key,
                        };
                        for neighbor in edge.faces() {
                            visit(neighbor);
                        }
                    }
                    continue;
                }

                for vert_key in face.verts() {
                    for neighbor in self.vertex(vert_key).faces() {
                        visit(neighbor);
                    }
                }
            }
            boundary
        })
    }

    fn vertex(&self, key: VertKey) -> VertRef<'a> {
        VertRef {
            topo: self.selection.topo,
            attrs: self.selection.attrs,
            key,
        }
    }

    fn face(&self, key: FaceKey) -> FaceRef<'a> {
        FaceRef {
            topo: self.selection.topo,
            attrs: self.selection.attrs,
            key,
        }
    }
}
