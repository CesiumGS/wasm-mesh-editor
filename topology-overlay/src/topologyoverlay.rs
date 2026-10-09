use std::ops::Range;
use std::sync::mpsc::Receiver;

use event_emitter::Event;
use mesh_core::{
    ComponentKey, ComponentType, EdgeKey, FaceKey, Mesh, MeshChange, PerComponentType, VertKey,
};
use slotmap::{Key, SecondaryMap};

/// A buffer and the entries changed by the latest update.
#[derive(Default)]
pub struct OverlayBuffer<T> {
    pub data: Vec<T>,
    /// Ranges index `data`, not bytes. They may overlap or be out of order;
    /// the renderer can merge them before uploading.
    pub dirty_ranges: Vec<Range<usize>>,
}

impl<T> OverlayBuffer<T> {
    fn new(data: Vec<T>) -> Self {
        Self {
            data,
            dirty_ranges: Vec::new(),
        }
    }
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

impl OverlayBuffers {
    fn new(
        mesh: &Mesh,
        vertices: &[VertKey],
        edges: &[EdgeKey],
        faces: &[FaceKey],
        vertex_indices: &SecondaryMap<VertKey, u32>,
        edge_indices: &SecondaryMap<EdgeKey, u32>,
        face_indices: &SecondaryMap<FaceKey, u32>,
    ) -> Self {
        let positions = vertices
            .iter()
            .map(|&key| mesh.attributes().positions[key].extend(0.0).to_array())
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

        let selected = mesh.selection();
        let mut selection = PerComponentType::default();
        selection[ComponentType::Vertex] = OverlayBuffer::new(vec![0; vertices.len()]);
        selection[ComponentType::Edge] = OverlayBuffer::new(vec![0; edges.len()]);
        selection[ComponentType::Face] = OverlayBuffer::new(vec![0; faces.len()]);

        // Generally speaking, these loops do nothing (since TopologyOverlay is usually created over a new mesh, with nothing selected yet)
        for key in selected.verts() {
            selection[ComponentType::Vertex].data[vertex_indices[key] as usize] = 255;
        }

        for key in selected.edges() {
            selection[ComponentType::Edge].data[edge_indices[key] as usize] = 255;
        }

        for key in selected.faces() {
            selection[ComponentType::Face].data[face_indices[key] as usize] = 255;
        }

        Self {
            positions: OverlayBuffer::new(positions),
            edges: OverlayBuffer::new(edge_endpoints),
            triangles: OverlayBuffer::new(triangles),
            selection,
            full_refresh: true,
        }
    }
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
    pub fn new(mesh: &mut Mesh) -> Self {
        let mesh_changes = mesh.subscribe();
        let topology = mesh.topology();

        let buffer_index_to_vertex_key: Vec<VertKey> = topology.verts().collect();
        let buffer_index_to_edge_key: Vec<EdgeKey> = topology.edges().collect();
        let buffer_index_to_face_key: Vec<FaceKey> = topology.faces().collect();

        let vertex_key_to_buffer_index = build_key_to_buffer_index(&buffer_index_to_vertex_key);
        let edge_key_to_buffer_index = build_key_to_buffer_index(&buffer_index_to_edge_key);
        let face_key_to_buffer_index = build_key_to_buffer_index(&buffer_index_to_face_key);

        let buffers = OverlayBuffers::new(
            mesh,
            &buffer_index_to_vertex_key,
            &buffer_index_to_edge_key,
            &buffer_index_to_face_key,
            &vertex_key_to_buffer_index,
            &edge_key_to_buffer_index,
            &face_key_to_buffer_index,
        );

        Self {
            buffers,
            vertex_key_to_buffer_index,
            edge_key_to_buffer_index,
            face_key_to_buffer_index,
            buffer_index_to_vertex_key,
            buffer_index_to_edge_key,
            buffer_index_to_face_key,
            initial_upload_pending: true,
            mesh_changes,
        }
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

fn build_key_to_buffer_index<K: Key>(keys: &[K]) -> SecondaryMap<K, u32> {
    keys.iter()
        .enumerate()
        .map(|(index, &key)| (key, index as u32))
        .collect()
}

#[cfg(test)]
#[path = "../tests/unit/topologyoverlay.rs"]
mod tests;
