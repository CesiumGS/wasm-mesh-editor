//! The boundary of a selection.

use std::collections::HashSet;
use std::sync::OnceLock;

use super::SelectionView;
use crate::{FaceKey, FaceRef, VertKey, VertRef};

/// The boundary sets stored for the lifetime of a selection. Each set is computed
/// lazily when requested and invalidated when the selection changes.
///
/// A fully selected mesh has no boundary vertices, which is a valid computed
/// result.
#[derive(Default)]
pub(super) struct BoundaryCache {
    /// Selected vertices belonging to a face that has at least one unselected vertex.
    pub(super) vertices: OnceLock<HashSet<VertKey>>,
    /// Selected vertices adjacent to the boundary, excluding the boundary itself.
    pub(super) inner_vertices: OnceLock<HashSet<VertKey>>,
    /// Unselected vertices adjacent to the boundary.
    pub(super) outer_vertices: OnceLock<HashSet<VertKey>>,
    /// Faces touching one or more boundary vertices and one or more inner vertices.
    pub(super) inner_faces: OnceLock<HashSet<FaceKey>>,
    /// Faces touching one or more boundary vertices and one or more outer vertices.
    pub(super) outer_faces: OnceLock<HashSet<FaceKey>>,
}

/// A view of a selection representing the vertices on its boundary, as well as
/// the sets of vertices adjacent to the boundary (the "inner" and "outer" rings).
/// This is a read-only view: it does not modify the selection itself, but can be
/// used to implement grow and shrink operations, or by operations that need
/// boundary information, such as recomputing vertex normals after translating
/// a selection.
///
/// The boundary and its rings are computed lazily when requested and reused
/// until invalidated by a selection change. Translating vertices does not change
/// which components belong to these sets, so position edits leave them cached.
///
/// The inner and outer rings use edge adjacency: a vertex must share an edge
/// with a boundary vertex, not merely belong to the same face.
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

    /// The vertices on the boundary of the selection: any vertex that is selected
    /// and belongs to a face that has at least one unselected vertex.
    pub fn vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.boundary_vertices().iter().copied()
    }

    /// Selected vertices adjacent to the boundary (the inner ring), excluding
    /// the boundary itself. These vertices remain selected after vertex-level shrink.
    pub fn inner_vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.vertex_ring(true).iter().copied()
    }

    /// Unselected vertices adjacent to the boundary (the outer ring).
    /// These are the vertices added by vertex-level grow.
    pub fn outer_vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.vertex_ring(false).iter().copied()
    }

    /// The faces on the inner side of the selection boundary: faces touching
    /// one or more boundary vertices and one or more inner vertices.
    pub fn inner_faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        self.cache
            .inner_faces
            .get_or_init(|| self.collect_faces(self.vertex_ring(true)))
            .iter()
            .copied()
    }

    /// The faces on the outer side of the selection boundary: faces touching
    /// one or more boundary vertices and one or more outer vertices.
    pub fn outer_faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        self.cache
            .outer_faces
            .get_or_init(|| self.collect_faces(self.vertex_ring(false)))
            .iter()
            .copied()
    }

    /// Lazily compute the boundary vertices when requested. A vertex is on the boundary
    /// iff it is selected and belongs to a face that has at least one unselected vertex.
    fn boundary_vertices(&self) -> &HashSet<VertKey> {
        self.cache.vertices.get_or_init(|| {
            let mut boundary = HashSet::new();
            let mut visited_faces = HashSet::new();
            for key in self.selection.verts() {
                let vertex = VertRef {
                    topo: self.selection.topo,
                    attrs: self.selection.attrs,
                    key,
                };

                for face_key in vertex.faces() {
                    if !visited_faces.insert(face_key) {
                        continue;
                    }

                    let face = FaceRef {
                        topo: self.selection.topo,
                        attrs: self.selection.attrs,
                        key: face_key,
                    };

                    if face.verts().any(|key| !self.selection.contains(key)) {
                        boundary.extend(face.verts().filter(|&key| self.selection.contains(key)));
                    }
                }
            }

            boundary
        })
    }

    /// Lazily collect the neighbors of the boundary, excluding the boundary itself.
    /// `selected` filters membership: `true` collects selected neighbors (the inner
    /// ring), while `false` collects unselected neighbors (the outer ring).
    fn vertex_ring(&self, selected: bool) -> &HashSet<VertKey> {
        let cache = if selected {
            &self.cache.inner_vertices
        } else {
            &self.cache.outer_vertices
        };

        cache.get_or_init(|| {
            let boundary = self.boundary_vertices();
            let mut ring = HashSet::new();

            for &key in boundary {
                let vertex = VertRef {
                    topo: self.selection.topo,
                    attrs: self.selection.attrs,
                    key,
                };

                ring.extend(vertex.neighbors().filter(|key| {
                    !boundary.contains(key) && self.selection.contains(*key) == selected
                }));
            }

            ring
        })
    }

    /// Collect faces touching both a boundary vertex and a vertex in `included`.
    /// Passing the inner vertices collects the inner faces; passing the outer
    /// vertices collects the outer faces. Each face is collected once.
    fn collect_faces(&self, included: &HashSet<VertKey>) -> HashSet<FaceKey> {
        let mut faces = HashSet::new();
        let mut visited = HashSet::new();

        for key in self.vertices() {
            let vertex = VertRef {
                topo: self.selection.topo,
                attrs: self.selection.attrs,
                key,
            };

            for face_key in vertex.faces() {
                if !visited.insert(face_key) {
                    continue;
                }

                let face = FaceRef {
                    topo: self.selection.topo,
                    attrs: self.selection.attrs,
                    key: face_key,
                };

                if face.verts().any(|key| included.contains(&key)) {
                    faces.insert(face_key);
                }
            }
        }
        faces
    }
}
