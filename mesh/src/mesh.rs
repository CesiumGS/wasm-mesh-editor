//! The [`Mesh`] aggregate: topology, attributes, selection, and hidden components.

mod build;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use event_emitter::{Event, EventEmitter};
use glam::{DVec3, Vec3};

use crate::geometry::corner_angle;
use crate::selection::{SelectionBoundary, SelectionState};
use crate::{
    Aabb, AttributeChange, AttributeError, AttributeKey, AttributeValue, Attributes, ComponentKey,
    ComponentMask, ComponentRef, ComponentSet, EdgeKey, EdgeRef, FaceKey, FaceRef, Hidden,
    HideOptions, LoopKey, MeshChange, Selection, SelectionView, Topology, VertKey, VertRef,
};

pub use build::{EdgeAttributeBuffer, MeshBuffers, MeshBuildError};

/// A headless, editable mesh: connectivity, geometry attributes, selection, and hidden components.
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct Mesh {
    pub(crate) topology: Topology,
    pub(crate) attributes: Attributes,
    pub(crate) selection: SelectionState,
    pub(crate) hidden: ComponentSet,
    interactable_components: ComponentMask,
    changes: EventEmitter<MeshChange>,

    /// Face normals cached per smooth update; storage is reused across updates
    /// just to avoid reallocating the storage on each update.
    face_normals: HashMap<FaceKey, DVec3>,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            topology: Topology::default(),
            attributes: Attributes::default(),
            selection: SelectionState::default(),
            hidden: ComponentSet::default(),
            interactable_components: ComponentMask::VERTEX,
            changes: EventEmitter::default(),
            face_normals: HashMap::new(),
        }
    }
}

impl Mesh {
    /// Creates an empty mesh with interactable vertices.
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

    /// Local-space bounds of every vertex, or `None` for an empty mesh.
    pub fn bounds(&self) -> Option<Aabb> {
        self.selection.mesh_bounds(&self.topology, &self.attributes)
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
            attrs: &self.attributes,
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
        if delta == Vec3::ZERO || self.selection.components.verts.is_empty() {
            return;
        }

        let Self {
            topology,
            attributes,
            selection,
            ..
        } = self;

        let positions = attributes.positions_mut();
        for &key in selection.components.verts.iter() {
            positions[key] += delta;
        }

        selection.translate_bounds(delta);

        let boundary = SelectionBoundary::new(selection, topology);
        let loops = Rc::clone(boundary.mixed_face_loops());
        Self::recompute_flat_normals(topology, attributes, boundary.mixed_faces().iter().copied());

        let verts = Rc::clone(&selection.components.verts);
        self.emit_position_change(verts);
        self.emit_normal_change(loops);
    }

    /// Writes `values` to the attribute called `name`, built-in or custom, ignoring keys that
    /// aren't in the mesh. Writing positions is a special case that triggers normals recomputation
    /// on the faces using the written vertices. For writing positions, prefer
    /// [`Self::translate_selected`], which amortizes  the overhead frame-to-frame by caching
    /// derived sets of the selection.
    ///
    /// # Errors
    /// Like [`Attributes::values`]. Writes nothing on error.
    pub fn set_attribute_values<K: AttributeKey, T: AttributeValue>(
        &mut self,
        name: &str,
        values: &[(K, T)],
    ) -> Result<(), AttributeError> {
        let map = self.attributes.values_mut::<K, T>(name)?;

        // Write the provided values to the attribute map, collecting the keys that were successfully written.
        let written: Vec<K> = values
            .iter()
            .filter_map(|&(key, value)| {
                *map.get_mut(key)? = value;
                Some(key)
            })
            .collect();

        if written.is_empty() {
            return Ok(());
        }

        let keys = written.iter().map(|key| key.data());
        match name {
            Attributes::POSITION => {
                let verts: HashSet<_> = keys.map(VertKey::from).collect();
                let faces: HashSet<_> = verts
                    .iter()
                    .flat_map(|&vert| self.topology.vert_faces(vert))
                    .collect();
                let loops = faces
                    .iter()
                    .flat_map(|&face| self.topology.face_loops(face))
                    .collect();

                Self::recompute_flat_normals(
                    &self.topology,
                    &mut self.attributes,
                    faces.iter().copied(),
                );

                self.selection.invalidate_bounds();
                self.emit_position_change(Rc::new(verts));
                self.emit_normal_change(Rc::new(loops));
            }
            Attributes::NORMAL => {
                self.emit_normal_change(Rc::new(keys.map(LoopKey::from).collect()));
            }
            Attributes::TEXCOORD_0 => {
                let change = AttributeChange::Uv(Rc::new(keys.map(LoopKey::from).collect()));
                self.changes.emit(MeshChange::Attributes(change));
            }
            _ => {}
        }

        Ok(())
    }

    /// Recompute flat normals on the selected faces.
    pub fn shade_flat(&mut self) {
        let Self {
            topology,
            attributes,
            selection,
            ..
        } = self;

        Self::recompute_flat_normals(
            topology,
            attributes,
            selection.components.faces.iter().copied(),
        );

        let loops = selection
            .components
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
            selection.components.verts.iter().copied(),
        );

        self.emit_normal_change(Rc::new(loops));
    }

    /// The types of components that can currently be directly modified (i.e. selected, hidden, etc).
    /// Note: components not included in this mask may still be indirectly affected by operations on included components.
    pub fn interactable_components(&self) -> ComponentMask {
        self.interactable_components
    }

    /// Sets the types of components that can be directly modified (i.e. selected, hidden, etc).
    ///
    /// Notes:
    /// 1. Setting this may have side effects on the mesh state (like what is selected).
    /// 2. Components not included in this mask may still be indirectly affected by operations on included components.
    pub fn set_interactable_components(&mut self, components: ComponentMask) {
        if self.interactable_components == components {
            return;
        }

        self.interactable_components = components;
        self.selection_mut().rebuild();
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
            interactable: self.interactable_components,
            changes: &mut self.changes,
        }
    }

    pub fn hidden(&self) -> &ComponentSet {
        todo!()
    }

    pub fn hidden_mut(&mut self) -> Hidden<'_> {
        todo!()
    }

    /// Hides the selected components of interactable types, then deselects them.
    pub fn hide_selected(&mut self, options: HideOptions) {
        todo!()
    }

    /// Shows the selected components of interactable types.
    pub fn show_selected(&mut self, options: HideOptions) {
        todo!()
    }

    /// Subscribes to future mesh changes in emission order. Use
    /// [`Receiver::try_iter`] or [`Receiver::try_recv`] to process changes. Drop it to unsubscribe.
    /// Read initial state from the mesh; construction and past changes are not replayed.
    /// Notifications identify affected keys, whose current state may have changed again before processing.
    /// Translation events share vertex and loop-key snapshots; selection edits leave queued snapshots intact.
    pub fn subscribe(&mut self) -> Receiver<Event<MeshChange>> {
        self.changes.subscribe()
    }

    fn emit_position_change(&mut self, verts: Rc<HashSet<VertKey>>) {
        self.changes
            .emit(MeshChange::Attributes(AttributeChange::Position(verts)));
    }

    fn emit_normal_change(&mut self, loops: Rc<Vec<LoopKey>>) {
        let change = AttributeChange::Normal(loops);

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

            let normals = attributes.normals_mut();
            for loop_key in topology.face_loops(key) {
                normals.insert(loop_key, normal);
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
            let positions = attributes.positions();
            let origin = positions[key].as_dvec3();
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
                let previous = positions[previous_vertex].as_dvec3() - origin;
                let next = positions[next_vertex].as_dvec3() - origin;

                normal += face_normal * corner_angle(previous, next, face_normal);
            }

            let normal = normal.normalize_or_zero().as_vec3();

            let normals = attributes.normals_mut();
            for loop_key in topology.vert_loops(key) {
                normals.insert(loop_key, normal);
                loops.push(loop_key);
            }
        }

        loops
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh.rs"]
mod tests;
