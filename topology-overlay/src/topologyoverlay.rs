use std::ops::Range;
use std::sync::mpsc::Receiver;

use event_emitter::Event;
use mesh_core::{
    ComponentKey, ComponentType, EdgeKey, FaceKey, Mesh, MeshChange, PerComponentType, VertKey,
};
use slotmap::SecondaryMap;

/// A buffer and the entries changed by the latest update.
pub struct OverlayBuffer<T> {
    pub data: Vec<T>,
    /// Ranges index `data`, not bytes. They may overlap or be out of order;
    /// the renderer can merge them before uploading.
    pub dirty_ranges: Vec<Range<usize>>,
}

/// Mesh data packed for rendering.
pub struct OverlayBuffers {
    /// One `[x, y, z, 0.0]` per vertex (RGBA32F). Point instances index this directly.
    pub positions: OverlayBuffer<[f32; 4]>,
    /// Two position indices per edge (RG32UI).
    pub edges: OverlayBuffer<[u32; 2]>,
    /// Three position indices and a face index per triangle (RGBA32UI).
    pub triangles: OverlayBuffer<[u32; 4]>,
    /// Selection by vertex, edge, or face index: 0 unselected, 255 selected.
    pub selection: PerComponentType<OverlayBuffer<u8>>,
    /// Upload all buffers and refresh picking maps, regardless of dirty ranges.
    /// True on the first update and after topology edits.
    pub full_refresh: bool,
}

/// Translates mesh data into a GPU-friendly representation. This is not the renderer, just a translation layer.
/// It triangulates the mesh and creates + maintains CPU-side arrays, but does not manage any GPU resources itself.
#[allow(dead_code)]
pub struct TopologyOverlay {
    buffers: OverlayBuffers,
    // Structures for mapping between mesh keys and buffer keys
    vertex_key_to_buffer_index: SecondaryMap<VertKey, u32>,
    edge_key_to_buffer_index: SecondaryMap<EdgeKey, u32>,
    face_key_to_buffer_index: SecondaryMap<FaceKey, u32>,
    // Structures for mapping between buffer indices and mesh keys (e.g. for picking)
    buffer_index_to_vertex_key: Vec<VertKey>,
    buffer_index_to_edge_key: Vec<EdgeKey>,
    buffer_index_to_face_key: Vec<FaceKey>,
    initial_upload_pending: bool,
    mesh_changes: Receiver<Event<MeshChange>>,
}

impl TopologyOverlay {
    /// Builds buffers from the mesh and subscribes to changes.
    pub fn new(_mesh: &mut Mesh) -> Self {
        todo!()
    }

    /// Drains mesh changes and returns buffers to upload, or `None` if nothing changed.
    /// Pass the same mesh used in `new`. Topology edits rebuild all buffers.
    /// Consume the result before the next update, which clears the previous dirty ranges.
    pub fn update(&mut self, _mesh: &Mesh) -> Option<&OverlayBuffers> {
        todo!()
    }

    /// Returns the mesh key for a buffer index, or `None` if out of bounds.
    /// Faces use the triangle's face index. Indices can change after topology edits.
    pub fn component_key(&self, _kind: ComponentType, _index: u32) -> Option<ComponentKey> {
        todo!()
    }
}
