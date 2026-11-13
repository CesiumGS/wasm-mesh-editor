//! JS adapters for [`Mesh`](crate::Mesh).

use bytemuck::Pod;
use glam::{Vec2, Vec3};
use wasm_bindgen::prelude::*;

use crate::{AttributeBuffer, Mesh, MeshBuffers};

/// Whether a custom attribute has one value per vertex or one per face corner.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeDomain {
    /// One value per vertex, in `positions` order.
    Vertex = 0,
    /// One value per face corner, in `indices` order.
    Corner = 1,
}

/// A named custom attribute to pass to `Mesh.fromBuffers`. Create one with a per-type
/// constructor. Passing it to `fromBuffers` consumes it.
#[wasm_bindgen]
pub struct CustomAttribute {
    name: String,
    domain: AttributeDomain,
    values: AttributeBuffer,
}

#[wasm_bindgen]
impl CustomAttribute {
    /// Float values with 1 to 4 components each (scalar, vec2, vec3 or vec4), flattened.
    /// Throws if `componentCount` is out of range or doesn't divide the length of `values`.
    pub fn float32(
        name: String,
        domain: AttributeDomain,
        #[wasm_bindgen(js_name = componentCount)] component_count: u32,
        values: Vec<f32>,
    ) -> Result<CustomAttribute, JsError> {
        let values = match component_count {
            1 => AttributeBuffer::F32(values),
            2 => AttributeBuffer::Vec2(vectors(&name, values)?),
            3 => AttributeBuffer::Vec3(vectors(&name, values)?),
            4 => AttributeBuffer::Vec4(vectors(&name, values)?),
            _ => {
                return Err(JsError::new(&format!(
                    "{name} componentCount {component_count} is not 1, 2, 3 or 4"
                )));
            }
        };

        Ok(Self {
            name,
            domain,
            values,
        })
    }

    /// Unsigned integer scalars.
    pub fn uint32(name: String, domain: AttributeDomain, values: Vec<u32>) -> CustomAttribute {
        Self {
            name,
            domain,
            values: AttributeBuffer::U32(values),
        }
    }

    /// Signed integer scalars.
    pub fn int32(name: String, domain: AttributeDomain, values: Vec<i32>) -> CustomAttribute {
        Self {
            name,
            domain,
            values: AttributeBuffer::I32(values),
        }
    }
}

#[wasm_bindgen]
impl Mesh {
    /// Builds a mesh from flat buffers: xyz `positions` and `normals`, uv pairs, face `indices`,
    /// and custom attributes. Throws if the buffers are invalid. See [`MeshBuffers`] for each
    /// buffer's meaning.
    #[wasm_bindgen(js_name = fromBuffers)]
    pub fn from_flat_buffers(
        positions: Vec<f32>,
        indices: Vec<u32>,
        normals: Option<Vec<f32>>,
        uvs: Option<Vec<f32>>,
        #[wasm_bindgen(js_name = faceVertexCounts)] face_vertex_counts: Option<Vec<u32>>,
        attributes: Option<Vec<CustomAttribute>>,
    ) -> Result<Mesh, JsError> {
        let (vertex_attributes, corner_attributes) =
            split_by_domain(attributes.unwrap_or_default());

        let mesh = Mesh::from_buffers(MeshBuffers {
            positions: vectors::<Vec3>("positions", positions)?,
            normals: normals
                .map(|normals| vectors::<Vec3>("normals", normals))
                .transpose()?,
            uvs: uvs.map(|uvs| vectors::<Vec2>("uvs", uvs)).transpose()?,
            indices,
            face_vertex_counts,
            vertex_attributes,
            corner_attributes,
        })?;

        Ok(mesh)
    }
}

type NamedBuffers = Vec<(String, AttributeBuffer)>;

/// Separates attributes into `MeshBuffers`' vertex and corner lists, keeping their order.
fn split_by_domain(attributes: Vec<CustomAttribute>) -> (NamedBuffers, NamedBuffers) {
    let mut vertex_attributes = Vec::new();
    let mut corner_attributes = Vec::new();

    for attribute in attributes {
        let list = match attribute.domain {
            AttributeDomain::Vertex => &mut vertex_attributes,
            AttributeDomain::Corner => &mut corner_attributes,
        };
        list.push((attribute.name, attribute.values));
    }

    (vertex_attributes, corner_attributes)
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

    // Reinterpreting fails if the capacity isn't a whole number of vectors, or if `V` is more
    // aligned than `f32` (like `Vec4`); copy then.
    Ok(bytemuck::allocation::try_cast_vec(components)
        .unwrap_or_else(|(_, components)| bytemuck::allocation::pod_collect_to_vec(&components)))
}
