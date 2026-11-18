use std::ops::{Index, IndexMut, Range};
use std::sync::mpsc::Receiver;

use event_emitter::Event;
use mesh::{
    AttributeChange, ComponentKeys, ComponentSetChange, ComponentType, EdgeKey, FaceKey, Mesh,
    MeshChange, VertKey,
};
use slotmap::{Key, SecondaryMap};

/// A buffer and the entries changed by the latest update.
#[derive(Default)]
pub struct OverlayBuffer<T> {
    pub data: Vec<T>,
    /// The smallest range of `data` (element indices, not bytes) covering every entry changed by
    /// the latest update, or `None` if nothing changed. May include unchanged entries between
    /// changed ones.
    pub dirty_range: Option<Range<usize>>,
}

impl<T> OverlayBuffer<T> {
    fn new(data: Vec<T>) -> Self {
        Self {
            data,
            dirty_range: None,
        }
    }

    /// Grows `dirty_range` to also cover `range`. Does nothing if `range` is empty.
    fn extend_dirty_range(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }

        self.dirty_range = Some(match self.dirty_range.take() {
            Some(dirty) => dirty.start.min(range.start)..dirty.end.max(range.end),
            None => range,
        });
    }
}

/// Collects indices in a loop into the smallest covering range, to apply to a buffer once.
struct DirtyRangeBuilder {
    start: usize,
    end: usize,
}

impl DirtyRangeBuilder {
    fn new() -> Self {
        Self {
            start: usize::MAX,
            end: 0,
        }
    }

    fn include(&mut self, index: usize) {
        self.start = self.start.min(index);
        self.end = self.end.max(index + 1);
    }

    /// The covering range, or an empty range if no index was included.
    fn build(self) -> Range<usize> {
        self.start..self.end
    }
}

bitflags::bitflags! {
    /// Per-component state packed into one byte of a flag buffer.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ComponentFlags: u8 {
        const SELECTED = 0b01;
        const HIDDEN = 0b10;
    }
}

/// [`ComponentFlags`] buffers by component type, one byte per component.
#[derive(Default)]
pub struct ComponentFlagBuffers {
    pub verts: OverlayBuffer<u8>,
    pub edges: OverlayBuffer<u8>,
    pub faces: OverlayBuffer<u8>,
}

impl Index<ComponentType> for ComponentFlagBuffers {
    type Output = OverlayBuffer<u8>;

    fn index(&self, kind: ComponentType) -> &Self::Output {
        match kind {
            ComponentType::Vertex => &self.verts,
            ComponentType::Edge => &self.edges,
            ComponentType::Face => &self.faces,
        }
    }
}

impl IndexMut<ComponentType> for ComponentFlagBuffers {
    fn index_mut(&mut self, kind: ComponentType) -> &mut Self::Output {
        match kind {
            ComponentType::Vertex => &mut self.verts,
            ComponentType::Edge => &mut self.edges,
            ComponentType::Face => &mut self.faces,
        }
    }
}

/// Mesh data packed for rendering.
#[derive(Default)]
pub struct OverlayBuffers {
    /// One `[x, y, z, 0.0]` per vertex. Point instances index this directly.
    pub positions: OverlayBuffer<[f32; 4]>,
    /// Two position indices per edge.
    pub edges: OverlayBuffer<[u32; 2]>,
    /// Three position indices and a face index per triangle.
    pub triangles: OverlayBuffer<[u32; 4]>,
    /// [`ComponentFlags`] by vertex, edge, or face index.
    pub flags: ComponentFlagBuffers,
    /// Mesh component keys in the same order as each buffer, mapping buffer elements to mesh components. For faces,
    /// an entry here corresponds to a face (the 4th element) in the `triangles` buffer entry.
    /// Changes only on a full refresh.
    pub component_keys: ComponentKeys,
    /// Upload all buffers and re-read `component_keys`. Dirty ranges are `None` when this is set.
    /// True on the first update and after topology edits.
    pub full_refresh: bool,
}

impl OverlayBuffers {
    fn new(
        mesh: &Mesh,
        component_keys: ComponentKeys,
        vertex_indices: &SecondaryMap<VertKey, u32>,
        edge_indices: &SecondaryMap<EdgeKey, u32>,
        face_indices: &SecondaryMap<FaceKey, u32>,
    ) -> Self {
        let ComponentKeys {
            verts: vertices,
            edges,
            faces,
        } = &component_keys;

        let mesh_positions = mesh.attributes().positions();
        let positions = vertices
            .iter()
            .map(|&key| mesh_positions[key].extend(0.0).to_array())
            .collect();

        let edge_endpoints = edges
            .iter()
            .map(|&key| {
                mesh.edge(key)
                    .unwrap()
                    .verts()
                    .map(|key| vertex_indices[key])
            })
            .collect();

        let topology = mesh.topology();
        let triangle_count = topology.loop_count() - 2 * topology.face_count();
        let mut triangles = Vec::with_capacity(triangle_count);

        for (index, &key) in faces.iter().enumerate() {
            let face_index = index as u32;

            for triangle in mesh.face(key).unwrap().triangulation() {
                let [first, second, third] = triangle.map(|key| vertex_indices[key]);
                triangles.push([first, second, third, face_index]);
            }
        }

        let mut flags = ComponentFlagBuffers {
            verts: OverlayBuffer::new(vec![0; vertices.len()]),
            edges: OverlayBuffer::new(vec![0; edges.len()]),
            faces: OverlayBuffer::new(vec![0; faces.len()]),
        };

        // Usually does nothing, since overlays are typically created over new meshes.
        for (set, flag) in [
            (&*mesh.selection(), ComponentFlags::SELECTED),
            (mesh.hidden_components(), ComponentFlags::HIDDEN),
        ] {
            for key in set.verts() {
                flags.verts.data[vertex_indices[key] as usize] |= flag.bits();
            }

            for key in set.edges() {
                flags.edges.data[edge_indices[key] as usize] |= flag.bits();
            }

            for key in set.faces() {
                flags.faces.data[face_indices[key] as usize] |= flag.bits();
            }
        }

        Self {
            positions: OverlayBuffer::new(positions),
            edges: OverlayBuffer::new(edge_endpoints),
            triangles: OverlayBuffer::new(triangles),
            flags,
            component_keys,
            full_refresh: true,
        }
    }

    fn clear_changes(&mut self) {
        self.full_refresh = false;
        self.positions.dirty_range = None;
        self.edges.dirty_range = None;
        self.triangles.dirty_range = None;

        for kind in [
            ComponentType::Vertex,
            ComponentType::Edge,
            ComponentType::Face,
        ] {
            self.flags[kind].dirty_range = None;
        }
    }
}

#[derive(Default)]
enum PendingChanges {
    #[default]
    None,
    Partial,
    Full,
}

impl PendingChanges {
    /// Upgrades a `PendingChanges` from `None` to `Partial` if it hasn't been set yet.
    /// Does nothing if the `PendingChanges` is already `Partial` or `Full`.
    fn request_partial(&mut self) {
        if matches!(self, Self::None) {
            *self = Self::Partial;
        }
    }
}

/// Translates mesh data into a GPU-friendly representation. This is not the renderer, just a translation layer.
/// It triangulates the mesh and creates + maintains CPU-side arrays, but does not manage any GPU resources itself.
pub struct TopologyOverlay {
    buffers: OverlayBuffers,
    // Structures for mapping between mesh keys and buffer keys
    vertex_key_to_buffer_index: SecondaryMap<VertKey, u32>,
    edge_key_to_buffer_index: SecondaryMap<EdgeKey, u32>,
    face_key_to_buffer_index: SecondaryMap<FaceKey, u32>,
    pending_upload: PendingChanges,
    mesh_changes: Receiver<Event<MeshChange>>,
}

impl TopologyOverlay {
    /// Builds buffers, subscribes to changes, and queues a full upload for `update`.
    pub fn new(mesh: &mut Mesh) -> Self {
        let mut overlay = Self {
            buffers: OverlayBuffers::default(),
            vertex_key_to_buffer_index: SecondaryMap::new(),
            edge_key_to_buffer_index: SecondaryMap::new(),
            face_key_to_buffer_index: SecondaryMap::new(),
            pending_upload: PendingChanges::None,
            mesh_changes: mesh.subscribe(),
        };

        overlay.rebuild(mesh);
        overlay
    }

    /// Drains mesh changes and returns buffers to upload, or `None` if nothing changed.
    /// The first call returns a full upload, even if no changes have arrived.
    /// Pass the same mesh used in `new`. Topology edits rebuild all buffers.
    /// Consume the result before the next update, which clears the previous dirty ranges.
    pub fn update(&mut self, mesh: &Mesh) -> Option<&OverlayBuffers> {
        self.buffers.clear_changes();
        self.process_mesh_changes(mesh);

        let pending = std::mem::take(&mut self.pending_upload);
        if matches!(pending, PendingChanges::None) {
            return None;
        }

        if matches!(pending, PendingChanges::Full) {
            self.buffers.clear_changes();
            self.buffers.full_refresh = true;
        }

        Some(&self.buffers)
    }

    /// Borrows the packed buffers. Queued mesh changes are applied by [`Self::update`].
    pub fn buffers(&self) -> &OverlayBuffers {
        &self.buffers
    }

    /// The vertex's index in the `positions` buffer, or `None` if the buffers don't include it yet.
    pub fn vertex_buffer_index(&self, key: VertKey) -> Option<u32> {
        self.vertex_key_to_buffer_index.get(key).copied()
    }

    fn process_mesh_changes(&mut self, mesh: &Mesh) {
        while let Ok(event) = self.mesh_changes.try_recv() {
            match event.as_ref() {
                MeshChange::Topology(change) if !change.is_empty() => {
                    self.mesh_changes.try_iter().for_each(drop);
                    self.rebuild(mesh);
                    break;
                }
                MeshChange::Attributes(AttributeChange::Position(keys)) => {
                    self.update_positions(mesh, keys.iter().copied());
                }
                MeshChange::Selection(change) => {
                    self.update_flags(change, ComponentFlags::SELECTED);
                }
                MeshChange::Hidden(change) => self.update_flags(change, ComponentFlags::HIDDEN),
                _ => {}
            }
        }
    }

    fn update_positions(&mut self, mesh: &Mesh, keys: impl IntoIterator<Item = VertKey>) {
        let mut dirty = DirtyRangeBuilder::new();
        let positions = mesh.attributes().positions();

        for key in keys {
            let (Some(&index), Some(position)) =
                (self.vertex_key_to_buffer_index.get(key), positions.get(key))
            else {
                continue;
            };

            let index = index as usize;
            self.buffers.positions.data[index] = position.extend(0.0).to_array();
            dirty.include(index);
        }

        let dirty = dirty.build();
        if !dirty.is_empty() {
            self.buffers.positions.extend_dirty_range(dirty);
            self.pending_upload.request_partial();
        }
    }

    fn update_flags(&mut self, change: &ComponentSetChange, flag: ComponentFlags) {
        for (keys, on) in [(&change.added, true), (&change.removed, false)] {
            Self::set_flag(
                &mut self.buffers.flags.verts,
                &keys.verts,
                &self.vertex_key_to_buffer_index,
                flag,
                on,
            );
            Self::set_flag(
                &mut self.buffers.flags.edges,
                &keys.edges,
                &self.edge_key_to_buffer_index,
                flag,
                on,
            );
            Self::set_flag(
                &mut self.buffers.flags.faces,
                &keys.faces,
                &self.face_key_to_buffer_index,
                flag,
                on,
            );
        }

        if !change.is_empty() {
            self.pending_upload.request_partial();
        }
    }

    fn set_flag<K: Key>(
        buffer: &mut OverlayBuffer<u8>,
        keys: &[K],
        indices: &SecondaryMap<K, u32>,
        flag: ComponentFlags,
        on: bool,
    ) {
        let mut dirty = DirtyRangeBuilder::new();

        for &key in keys {
            let Some(&index) = indices.get(key) else {
                continue;
            };

            let index = index as usize;
            let mut flags = ComponentFlags::from_bits_retain(buffer.data[index]);
            flags.set(flag, on);
            buffer.data[index] = flags.bits();
            dirty.include(index);
        }

        buffer.extend_dirty_range(dirty.build());
    }

    fn rebuild(&mut self, mesh: &Mesh) {
        let topology = mesh.topology();
        let component_keys = ComponentKeys {
            verts: topology.verts().collect(),
            edges: topology.edges().collect(),
            faces: topology.faces().collect(),
        };

        self.vertex_key_to_buffer_index = build_key_to_buffer_index(&component_keys.verts);
        self.edge_key_to_buffer_index = build_key_to_buffer_index(&component_keys.edges);
        self.face_key_to_buffer_index = build_key_to_buffer_index(&component_keys.faces);
        self.buffers = OverlayBuffers::new(
            mesh,
            component_keys,
            &self.vertex_key_to_buffer_index,
            &self.edge_key_to_buffer_index,
            &self.face_key_to_buffer_index,
        );
        self.pending_upload = PendingChanges::Full;
    }
}

fn build_key_to_buffer_index<K: Key>(keys: &[K]) -> SecondaryMap<K, u32> {
    keys.iter()
        .enumerate()
        .map(|(index, &key)| (key, index as u32))
        .collect()
}

#[cfg(test)]
#[path = "../tests/unit/topologyoverlay.rs"]
mod tests;
