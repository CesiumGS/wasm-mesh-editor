//! The [`Mesh`] aggregate: topology, attributes, and selection.

mod build;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use event_emitter::{Event, EventEmitter};
use glam::{DVec3, Vec3};

use crate::geometry::corner_angle;
use crate::selection::{SelectionBoundary, SelectionState};
use crate::{
    AttributeChange, AttributeId, AttributeKeys, Attributes, ComponentKey, ComponentRef, EdgeKey,
    EdgeRef, FaceKey, FaceRef, LoopKey, MeshChange, Selection, SelectionView, Topology, VertKey,
    VertRef,
};

pub use build::{MeshBuffers, MeshBuildError};

/// A headless, editable mesh: connectivity, geometry attributes, and selection.
#[derive(Default)]
pub struct Mesh {
    pub(crate) topology: Topology,
    pub(crate) attributes: Attributes,
    pub(crate) selection: SelectionState,
    changes: EventEmitter<MeshChange>,

    /// Face normals cached per smooth update; storage is reused across updates
    /// just to avoid reallocating the storage on each update.
    face_normals: HashMap<FaceKey, DVec3>,
}

impl Mesh {
    /// Creates an empty mesh with selection at vertex level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs an editable mesh from vertex attribute and face index buffers.
    /// Input buffers should contain decoded, typed data (not raw binary data).
    /// Vertices that are duplicated in the attribute buffers are treated as distinct vertices in the mesh.
    ///
    /// # Errors
    /// Returns an error if [`MeshBuffers::validate`] rejects the input buffers.
    pub fn from_buffers(buffers: MeshBuffers) -> Result<Self, MeshBuildError> {
        build::from_buffers(buffers)
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// Returns a view of a vertex, edge, or face, or `None` for a missing handle.
    pub fn component<K: Into<ComponentKey>>(&self, key: K) -> Option<ComponentRef<'_>> {
        let key = key.into();
        if !self.topology.contains(key) {
            return None;
        }
        Some(ComponentRef::new(key, &self.topology, &self.attributes))
    }

    pub fn vert(&self, key: VertKey) -> Option<VertRef<'_>> {
        if !self.topology.verts.contains_key(key) {
            return None;
        }
        Some(VertRef {
            topo: &self.topology,
            attrs: &self.attributes,
            key,
        })
    }

    pub fn edge(&self, key: EdgeKey) -> Option<EdgeRef<'_>> {
        if !self.topology.edges.contains_key(key) {
            return None;
        }
        Some(EdgeRef {
            topo: &self.topology,
            key,
        })
    }

    /// Returns a reference to the face with the given key, if it exists.
    /// References wrap the underlying mesh component handle with access to both mesh topology and attributes.
    pub fn face(&self, key: FaceKey) -> Option<FaceRef<'_>> {
        if !self.topology.faces.contains_key(key) {
            return None;
        }
        Some(FaceRef {
            topo: &self.topology,
            attrs: &self.attributes,
            key,
        })
    }

    /// Translate the currently selected vertices by `delta`.
    /// TODO: provide richer options for recomputing normals flat (smooth, sharp edges, transforming custom normals or regions or smooth/flat)
    pub fn translate_selected(&mut self, delta: Vec3) {
        if delta == Vec3::ZERO || self.selection.verts.is_empty() {
            return;
        }

        let Self {
            topology,
            attributes,
            selection,
            ..
        } = self;

        for &key in selection.verts.iter() {
            attributes.positions[key] += delta;
        }

        let boundary = SelectionBoundary::new(selection, topology);
        let loops = Rc::clone(boundary.mixed_face_loops());
        Self::recompute_flat_normals(topology, attributes, boundary.mixed_faces().iter().copied());

        self.changes.emit(MeshChange::Attributes(AttributeChange {
            attribute: AttributeId::Position,
            keys: AttributeKeys::Vertices(Rc::clone(&selection.verts)),
        }));

        self.emit_normal_change(loops);
    }

    /// Recompute flat normals on the selected faces.
    pub fn shade_flat(&mut self) {
        let Self {
            topology,
            attributes,
            selection,
            ..
        } = self;

        Self::recompute_flat_normals(topology, attributes, selection.faces.iter().copied());

        let loops = selection
            .faces
            .iter()
            .flat_map(|&key| topology.face_loops(key))
            .collect();

        self.emit_normal_change(Rc::new(loops));
    }

    /// Recompute smooth normals at the selected vertices using all incident faces.
    pub fn shade_smooth(&mut self) {
        let Self {
            topology,
            attributes,
            selection,
            face_normals,
            ..
        } = self;

        let loops = Self::recompute_smooth_normals(
            topology,
            attributes,
            face_normals,
            selection.verts.iter().copied(),
        );

        self.emit_normal_change(Rc::new(loops));
    }

    pub fn selection(&self) -> SelectionView<'_> {
        SelectionView {
            state: &self.selection,
            topo: &self.topology,
            attrs: &self.attributes,
        }
    }

    pub fn selection_mut(&mut self) -> Selection<'_> {
        Selection {
            state: &mut self.selection,
            topo: &self.topology,
            attrs: &self.attributes,
            changes: &mut self.changes,
        }
    }

    /// Subscribes to future mesh changes in emission order. Use
    /// [`Receiver::try_iter`] or [`Receiver::try_recv`] to process changes. Drop it to unsubscribe.
    /// Read initial state from the mesh; construction and past changes are not replayed.
    /// Notifications identify affected keys, whose current state may have changed again before processing.
    /// Translation events share vertex and loop-key snapshots; selection edits leave queued snapshots intact.
    pub fn subscribe(&mut self) -> Receiver<Event<MeshChange>> {
        self.changes.subscribe()
    }

    fn emit_normal_change(&mut self, loops: Rc<Vec<LoopKey>>) {
        let change = AttributeChange {
            attribute: AttributeId::Normal,
            keys: AttributeKeys::Loops(loops),
        };

        if !change.is_empty() {
            self.changes.emit(MeshChange::Attributes(change));
        }
    }

    /// Write each face's normal to its corners. Keys must be live in this mesh.
    /// Zero-area faces receive zero normals.
    fn recompute_flat_normals(
        topology: &Topology,
        attributes: &mut Attributes,
        faces: impl IntoIterator<Item = FaceKey>,
    ) {
        for key in faces {
            let normal = FaceRef {
                topo: topology,
                attrs: attributes,
                key,
            }
            .normal();

            for loop_key in topology.face_loops(key) {
                attributes.normals.insert(loop_key, normal);
            }
        }
    }

    /// Average all incident face normals, weighted by the angle at each corner.
    /// Write the result to every corner at the requested vertices, with no sharp edges.
    /// Keys must be live in this mesh; undefined normals become zero. Returns the written corner keys.
    fn recompute_smooth_normals(
        topology: &Topology,
        attributes: &mut Attributes,
        face_normals: &mut HashMap<FaceKey, DVec3>,
        vertices: impl IntoIterator<Item = VertKey>,
    ) -> Vec<LoopKey> {
        face_normals.clear();
        let mut loops = Vec::new();

        for key in vertices {
            let origin = attributes.positions[key].as_dvec3();
            let mut normal = DVec3::ZERO;

            for loop_key in topology.vert_loops(key) {
                let corner = &topology.loops[loop_key];
                let face_normal = *face_normals.entry(corner.face).or_insert_with(|| {
                    FaceRef {
                        topo: topology,
                        attrs: attributes,
                        key: corner.face,
                    }
                    .normal()
                    .as_dvec3()
                    .normalize_or_zero()
                });

                if face_normal == DVec3::ZERO {
                    continue;
                }

                let previous_vertex = topology.loops[corner.prev].vert;
                let next_vertex = topology.loops[corner.next].vert;
                let previous = attributes.positions[previous_vertex].as_dvec3() - origin;
                let next = attributes.positions[next_vertex].as_dvec3() - origin;

                normal += face_normal * corner_angle(previous, next, face_normal);
            }

            let normal = normal.normalize_or_zero().as_vec3();

            for loop_key in topology.vert_loops(key) {
                attributes.normals.insert(loop_key, normal);
                loops.push(loop_key);
            }
        }

        loops
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh.rs"]
mod tests;
