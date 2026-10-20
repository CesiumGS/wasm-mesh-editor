//! JS adapters for [`Mesh`](crate::Mesh).

use bytemuck::Pod;
use glam::{Vec2, Vec3};
use wasm_bindgen::prelude::*;

use crate::{Mesh, MeshBuffers};

#[wasm_bindgen]
impl Mesh {
    /// Builds a mesh from flat buffers: xyz `positions` and `normals`, uv pairs, and face
    /// `indices`. Throws if the buffers are invalid. See [`MeshBuffers`] for each buffer's meaning.
    #[wasm_bindgen(js_name = fromBuffers)]
    pub fn from_flat_buffers(
        positions: Vec<f32>,
        indices: Vec<u32>,
        normals: Option<Vec<f32>>,
        uvs: Option<Vec<f32>>,
        #[wasm_bindgen(js_name = faceVertexCounts)] face_vertex_counts: Option<Vec<u32>>,
    ) -> Result<Mesh, JsError> {
        let mesh = Mesh::from_buffers(MeshBuffers {
            positions: vectors::<Vec3>("positions", positions)?,
            normals: normals
                .map(|normals| vectors::<Vec3>("normals", normals))
                .transpose()?,
            uvs: uvs.map(|uvs| vectors::<Vec2>("uvs", uvs)).transpose()?,
            indices,
            face_vertex_counts,
        })?;

        Ok(mesh)
    }
}

/// Groups flat components into vectors, reusing the buffer JS copied into wasm memory.
fn vectors<V: Pod>(name: &str, components: Vec<f32>) -> Result<Vec<V>, JsError> {
    let per_vector = size_of::<V>() / size_of::<f32>();

    if components.len() % per_vector != 0 {
        return Err(JsError::new(&format!(
            "{name} length {} is not a multiple of {per_vector}",
            components.len()
        )));
    }

    // Reinterpreting fails only if the capacity isn't a whole number of vectors; copy then.
    Ok(bytemuck::allocation::try_cast_vec(components)
        .unwrap_or_else(|(_, components)| bytemuck::cast_slice(&components).to_vec()))
}
