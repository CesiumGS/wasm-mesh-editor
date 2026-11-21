//! JS adapters for [`Mesh`](crate::Mesh).

use bytemuck::Pod;
use glam::{Vec2, Vec3};
use wasm_bindgen::prelude::*;

use crate::{AttributeBuffer, AttributeDomain, EdgeAttributeBuffer, Mesh, MeshBuffers};

/// A named custom attribute to pass to `Mesh.fromBuffers`. Create one with a per-type
/// constructor. Passing it to `fromBuffers` consumes it.
#[wasm_bindgen]
pub struct CustomAttribute {
    name: String,
    domain: AttributeDomain,
    values: AttributeBuffer,
    /// Only for `Edge` attributes, which require one.
    default: Option<AttributeBuffer>,
}

/// Wraps flat `$values` in the `AttributeBuffer` variant for `$component_count` components:
/// `$scalar` for 1, then the 2, 3 and 4 component vectors. Returns an error from the enclosing
/// function if the count is out of range or doesn't divide the length of `$values`.
macro_rules! by_component_count {
    (
        $name:expr, $component_count:expr, $values:expr,
        $scalar:ident, $vec2:ident, $vec3:ident, $vec4:ident
    ) => {
        match $component_count {
            1 => AttributeBuffer::$scalar($values),
            2 => AttributeBuffer::$vec2(vectors($name, $values)?),
            3 => AttributeBuffer::$vec3(vectors($name, $values)?),
            4 => AttributeBuffer::$vec4(vectors($name, $values)?),
            count => {
                return Err(JsError::new(&format!(
                    "{} componentCount {count} is not 1, 2, 3 or 4",
                    $name
                )));
            }
        }
    };
}

#[wasm_bindgen]
impl CustomAttribute {
    /// Float values with 1 to 4 components each (scalar, vec2, vec3 or vec4), flattened.
    /// `Edge` attributes require `defaultValue`, one value for the edges `edgeIndices` doesn't
    /// list, and other domains don't take one.
    /// Throws if `componentCount` is out of range or doesn't divide the length of `values`, or for
    /// a missing or unexpected `defaultValue`.
    pub fn float32(
        name: String,
        domain: AttributeDomain,
        #[wasm_bindgen(js_name = componentCount)] component_count: u32,
        values: Vec<f32>,
        #[wasm_bindgen(js_name = defaultValue)] default_value: Option<Vec<f32>>,
    ) -> Result<CustomAttribute, JsError> {
        let wrap = |values| -> Result<_, JsError> {
            Ok(by_component_count!(
                &name,
                component_count,
                values,
                F32,
                Vec2,
                Vec3,
                Vec4
            ))
        };
        let values = wrap(values)?;
        let default = default_value.map(wrap).transpose()?;

        Self::new(name, domain, values, default)
    }

    /// Unsigned integer values with 1 to 4 components each (scalar, uvec2, uvec3 or uvec4),
    /// flattened, like `float32`.
    pub fn uint32(
        name: String,
        domain: AttributeDomain,
        #[wasm_bindgen(js_name = componentCount)] component_count: u32,
        values: Vec<u32>,
        #[wasm_bindgen(js_name = defaultValue)] default_value: Option<Vec<u32>>,
    ) -> Result<CustomAttribute, JsError> {
        let wrap = |values| -> Result<_, JsError> {
            Ok(by_component_count!(
                &name,
                component_count,
                values,
                U32,
                UVec2,
                UVec3,
                UVec4
            ))
        };
        let values = wrap(values)?;
        let default = default_value.map(wrap).transpose()?;

        Self::new(name, domain, values, default)
    }

    /// Signed integer values with 1 to 4 components each (scalar, ivec2, ivec3 or ivec4),
    /// flattened, like `float32`.
    pub fn int32(
        name: String,
        domain: AttributeDomain,
        #[wasm_bindgen(js_name = componentCount)] component_count: u32,
        values: Vec<i32>,
        #[wasm_bindgen(js_name = defaultValue)] default_value: Option<Vec<i32>>,
    ) -> Result<CustomAttribute, JsError> {
        let wrap = |values| -> Result<_, JsError> {
            Ok(by_component_count!(
                &name,
                component_count,
                values,
                I32,
                IVec2,
                IVec3,
                IVec4
            ))
        };
        let values = wrap(values)?;
        let default = default_value.map(wrap).transpose()?;

        Self::new(name, domain, values, default)
    }
}

impl CustomAttribute {
    fn new(
        name: String,
        domain: AttributeDomain,
        values: AttributeBuffer,
        default: Option<AttributeBuffer>,
    ) -> Result<Self, JsError> {
        if (domain == AttributeDomain::Edge) != default.is_some() {
            return Err(JsError::new(&format!(
                "{name} defaultValue is required for Edge attributes and not allowed for others"
            )));
        }

        Ok(Self {
            name,
            domain,
            values,
            default,
        })
    }
}

#[wasm_bindgen]
impl Mesh {
    /// Builds a mesh from flat buffers: xyz `positions` and `normals`, uv pairs, face `indices`,
    /// vertex index pairs in `edgeIndices` for edge attributes, and custom attributes. Throws if
    /// the buffers are invalid. See [`MeshBuffers`] for each buffer's meaning.
    #[wasm_bindgen(js_name = fromBuffers)]
    pub fn from_flat_buffers(
        positions: Vec<f32>,
        indices: Vec<u32>,
        normals: Option<Vec<f32>>,
        uvs: Option<Vec<f32>>,
        #[wasm_bindgen(js_name = faceVertexCounts)] face_vertex_counts: Option<Vec<u32>>,
        attributes: Option<Vec<CustomAttribute>>,
        #[wasm_bindgen(js_name = edgeIndices)] edge_indices: Option<Vec<u32>>,
    ) -> Result<Mesh, JsError> {
        let mut buffers = MeshBuffers {
            positions: vectors::<f32, Vec3>("positions", positions)?,
            normals: normals
                .map(|normals| vectors::<f32, Vec3>("normals", normals))
                .transpose()?,
            uvs: uvs
                .map(|uvs| vectors::<f32, Vec2>("uvs", uvs))
                .transpose()?,
            indices,
            face_vertex_counts,
            edge_indices: edge_indices.unwrap_or_default(),
            ..MeshBuffers::default()
        };
        add_custom_attributes(&mut buffers, attributes.unwrap_or_default());

        Ok(Mesh::from_buffers(buffers)?)
    }
}

/// Adds attributes to `buffers`' list for their domain, keeping their order.
fn add_custom_attributes(buffers: &mut MeshBuffers, attributes: Vec<CustomAttribute>) {
    for CustomAttribute {
        name,
        domain,
        values,
        default,
    } in attributes
    {
        match domain {
            AttributeDomain::Vertex => buffers.vertex_attributes.push((name, values)),
            AttributeDomain::Corner => buffers.corner_attributes.push((name, values)),
            AttributeDomain::Face => buffers.face_attributes.push((name, values)),
            AttributeDomain::Edge => buffers.edge_attributes.push(EdgeAttributeBuffer {
                name,
                values,
                default: default.expect("CustomAttribute::new requires an edge default"),
            }),
        }
    }
}

/// Groups flat `S` components into vectors, reusing the buffer JS copied into wasm memory.
fn vectors<S: Pod, V: Pod>(name: &str, components: Vec<S>) -> Result<Vec<V>, JsError> {
    let per_vector = size_of::<V>() / size_of::<S>();

    if components.len() % per_vector != 0 {
        return Err(JsError::new(&format!(
            "{name} length {} is not a multiple of {per_vector}",
            components.len()
        )));
    }

    // Reinterpreting fails if the capacity isn't a whole number of vectors, or if `V` is more
    // aligned than `S` (like `Vec4`); copy then.
    Ok(bytemuck::allocation::try_cast_vec(components)
        .unwrap_or_else(|(_, components)| bytemuck::allocation::pod_collect_to_vec(&components)))
}
