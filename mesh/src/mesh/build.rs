//! Validated buffer import into a newly constructed mesh.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::mem::discriminant;

use glam::{Vec2, Vec3};

use super::Mesh;
use crate::{AttributeBuffer, Attributes, EdgeKey, FaceKey, FaceRef, Topology, Vert, VertKey};

/// Decoded vertex attribute and face index buffers describing a mesh to build.
/// File-format accessors, byte strides, and component types must be resolved first.
#[derive(Default)]
pub struct MeshBuffers {
    /// One entry per vertex; coincident positions are not welded.
    pub positions: Vec<Vec3>,
    /// Per-vertex shading normals, normalized before copying to loops.
    /// Zero denotes an undefined normal; non-finite components are rejected.
    /// Flat normals are generated if absent.
    pub normals: Option<Vec<Vec3>>,
    /// Per-vertex UVs, copied to loops; UV storage remains empty if absent.
    pub uvs: Option<Vec<Vec2>>,
    pub indices: Vec<u32>,
    /// Vertices per face, for n-gons; triangles are assumed when absent.
    pub face_vertex_counts: Option<Vec<u32>>,
    /// Named custom attributes with one value per vertex, in `positions` order.
    pub vertex_attributes: Vec<(String, AttributeBuffer)>,
    /// Named custom attributes with one value per face corner, in `indices` order,
    /// so corners of a shared vertex can have different values.
    pub corner_attributes: Vec<(String, AttributeBuffer)>,
    /// Pairs of vertex indices, each naming a different edge of a face, in either order.
    pub edge_indices: Vec<u32>,
    pub edge_attributes: Vec<EdgeAttributeBuffer>,
    /// Named custom attributes with one value per face, in face order.
    pub face_attributes: Vec<(String, AttributeBuffer)>,
}

/// A named custom attribute with one value per [`MeshBuffers::edge_indices`] pair.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeAttributeBuffer {
    pub name: String,
    pub values: AttributeBuffer,
    /// One value of the same type, for the edges that `edge_indices` doesn't list. `None` means
    /// zero.
    pub default: Option<AttributeBuffer>,
}

/// Invalid input buffers supplied to [`Mesh::from_buffers`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshBuildError {
    AttributeLengthMismatch {
        attribute: &'static str,
        expected: usize,
        actual: usize,
    },
    NonFiniteNormal {
        vertex: usize,
    },
    IncompleteTriangle {
        index_count: usize,
    },
    FaceTooSmall {
        face: usize,
        vertex_count: u32,
    },
    FaceVertexCountMismatch,
    VertexIndexOutOfBounds {
        corner: usize,
        index: u32,
        vertex_count: usize,
    },
    RepeatedFaceVertex {
        face: usize,
        index: u32,
    },
    IncompleteEdge {
        index_count: usize,
    },
    /// The pair at `edge` in `edge_indices` isn't an edge of any face.
    NotAnEdge {
        edge: usize,
    },
    /// The pair at `edge` in `edge_indices` names the same edge as an earlier pair.
    RepeatedEdge {
        edge: usize,
    },
    /// Expected one value per vertex, corner, edge pair or face, depending on the domain.
    CustomAttributeLengthMismatch {
        name: String,
        expected: usize,
        actual: usize,
    },
    /// Names must be unique across all domains.
    DuplicateAttributeName {
        name: String,
    },
    /// See [`Attributes::RESERVED_NAMES`].
    ReservedAttributeName {
        name: String,
    },
    /// An edge attribute's default isn't one value of the attribute's type.
    InvalidDefault {
        name: String,
    },
}

impl fmt::Display for MeshBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttributeLengthMismatch {
                attribute,
                expected,
                actual,
            } => write!(
                formatter,
                "{attribute} buffer has {actual} entries; expected {expected} (one per vertex)"
            ),
            Self::NonFiniteNormal { vertex } => write!(
                formatter,
                "normal at vertex {vertex} must contain only finite components"
            ),
            Self::IncompleteTriangle { index_count } => write!(
                formatter,
                "triangle index buffer length {index_count} is not divisible by three"
            ),
            Self::FaceTooSmall { face, vertex_count } => write!(
                formatter,
                "face {face} has {vertex_count} corners; at least three are required"
            ),
            Self::FaceVertexCountMismatch => write!(
                formatter,
                "face vertex counts do not cover the index buffer exactly"
            ),
            Self::VertexIndexOutOfBounds {
                corner,
                index,
                vertex_count,
            } => write!(
                formatter,
                "index {index} at corner {corner} is out of bounds for {vertex_count} vertices"
            ),
            Self::RepeatedFaceVertex { face, index } => {
                write!(
                    formatter,
                    "face {face} references vertex index {index} more than once"
                )
            }
            Self::IncompleteEdge { index_count } => write!(
                formatter,
                "edge index buffer length {index_count} is not divisible by two"
            ),
            Self::NotAnEdge { edge } => {
                write!(formatter, "edge pair {edge} is not an edge of any face")
            }
            Self::RepeatedEdge { edge } => {
                write!(formatter, "edge pair {edge} repeats an earlier edge")
            }
            Self::CustomAttributeLengthMismatch {
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "custom attribute {name:?} has {actual} values; expected {expected}"
            ),
            Self::DuplicateAttributeName { name } => {
                write!(
                    formatter,
                    "custom attribute name {name:?} is used more than once"
                )
            }
            Self::ReservedAttributeName { name } => write!(
                formatter,
                "custom attribute name {name:?} is reserved for a built-in attribute"
            ),
            Self::InvalidDefault { name } => write!(
                formatter,
                "edge attribute {name:?} default is not one value of the attribute's type"
            ),
        }
    }
}

impl std::error::Error for MeshBuildError {}

impl MeshBuffers {
    /// Checks buffer lengths, finite normals, face sizes, index bounds, repeated indices per face,
    /// that edge pairs are distinct face edges, and custom attribute names and defaults.
    /// Runs in linear time, temporarily storing the last-seen face ID per vertex
    /// to detect repeated indices without allocating per face.
    pub fn validate(&self) -> Result<(), MeshBuildError> {
        for (attribute, length) in [
            ("normals", self.normals.as_ref().map(Vec::len)),
            ("uvs", self.uvs.as_ref().map(Vec::len)),
        ] {
            let Some(actual) = length else {
                continue;
            };
            if actual != self.positions.len() {
                return Err(MeshBuildError::AttributeLengthMismatch {
                    attribute,
                    expected: self.positions.len(),
                    actual,
                });
            }
        }

        self.validate_custom_attributes()?;

        if let Some(normals) = &self.normals {
            for (vertex, normal) in normals.iter().enumerate() {
                if !normal.is_finite() {
                    return Err(MeshBuildError::NonFiniteNormal { vertex });
                }
            }
        }

        if self.face_vertex_counts.is_none() && self.indices.len() % 3 != 0 {
            return Err(MeshBuildError::IncompleteTriangle {
                index_count: self.indices.len(),
            });
        }

        if let Some(counts) = &self.face_vertex_counts {
            let mut remaining = self.indices.len();
            for (face, &vertex_count) in counts.iter().enumerate() {
                if vertex_count < 3 {
                    return Err(MeshBuildError::FaceTooSmall { face, vertex_count });
                }
                remaining = remaining
                    .checked_sub(vertex_count as usize)
                    .ok_or(MeshBuildError::FaceVertexCountMismatch)?;
            }
            if remaining != 0 {
                return Err(MeshBuildError::FaceVertexCountMismatch);
            }
        }

        let mut last_seen_face_by_vertex = vec![usize::MAX; self.positions.len()];
        let mut corner = 0;
        for (face, indices) in self.face_indices().enumerate() {
            for &index in indices {
                if index as usize >= self.positions.len() {
                    return Err(MeshBuildError::VertexIndexOutOfBounds {
                        corner,
                        index,
                        vertex_count: self.positions.len(),
                    });
                }
                if last_seen_face_by_vertex[index as usize] == face {
                    return Err(MeshBuildError::RepeatedFaceVertex { face, index });
                }
                last_seen_face_by_vertex[index as usize] = face;
                corner += 1;
            }
        }

        self.validate_edge_indices()
    }

    /// Checks that `edge_indices` holds pairs that are each a different edge of a face. Call after
    /// validating faces.
    fn validate_edge_indices(&self) -> Result<(), MeshBuildError> {
        if self.edge_indices.len() % 2 != 0 {
            return Err(MeshBuildError::IncompleteEdge {
                index_count: self.edge_indices.len(),
            });
        }

        if self.edge_indices.is_empty() {
            return Ok(());
        }

        let face_edges: HashSet<_> = self
            .face_indices()
            .flat_map(|indices| {
                (0..indices.len())
                    .map(|corner| edge_pair(indices[corner], indices[(corner + 1) % indices.len()]))
            })
            .collect();

        let mut seen = HashSet::new();
        for (edge, pair) in self.edge_indices.chunks_exact(2).enumerate() {
            let pair = edge_pair(pair[0], pair[1]);

            if !face_edges.contains(&pair) {
                return Err(MeshBuildError::NotAnEdge { edge });
            }
            if !seen.insert(pair) {
                return Err(MeshBuildError::RepeatedEdge { edge });
            }
        }

        Ok(())
    }

    /// Checks custom attribute lengths (`positions`, `indices`, `edge_indices` pairs or faces),
    /// edge attribute defaults, and that names are unique across all lists and not reserved.
    fn validate_custom_attributes(&self) -> Result<(), MeshBuildError> {
        let edge_attributes = self.edge_attributes.iter().map(|attribute| {
            (
                &attribute.name,
                &attribute.values,
                self.edge_indices.len() / 2,
            )
        });

        let attributes = with_length(&self.vertex_attributes, self.positions.len())
            .chain(with_length(&self.corner_attributes, self.indices.len()))
            .chain(edge_attributes)
            .chain(with_length(&self.face_attributes, self.face_count()));

        let mut names = HashSet::new();
        for (name, values, expected) in attributes {
            if Attributes::RESERVED_NAMES.contains(&name.as_str()) {
                return Err(MeshBuildError::ReservedAttributeName { name: name.clone() });
            }

            if !names.insert(name.as_str()) {
                return Err(MeshBuildError::DuplicateAttributeName { name: name.clone() });
            }

            if values.len() != expected {
                return Err(MeshBuildError::CustomAttributeLengthMismatch {
                    name: name.clone(),
                    expected,
                    actual: values.len(),
                });
            }
        }

        for EdgeAttributeBuffer {
            name,
            values,
            default,
        } in &self.edge_attributes
        {
            if default.as_ref().is_some_and(|default| {
                default.len() != 1 || discriminant(default) != discriminant(values)
            }) {
                return Err(MeshBuildError::InvalidDefault { name: name.clone() });
            }
        }

        Ok(())
    }

    fn face_count(&self) -> usize {
        self.face_vertex_counts
            .as_ref()
            .map_or(self.indices.len() / 3, Vec::len)
    }

    fn face_indices(&self) -> impl Iterator<Item = &[u32]> {
        (0..self.face_count()).scan(0, |offset, face| {
            let len = self
                .face_vertex_counts
                .as_ref()
                .map_or(3, |counts| counts[face] as usize);
            let indices = &self.indices[*offset..*offset + len];
            *offset += len;
            Some(indices)
        })
    }
}

pub(super) fn from_buffers(mut buffers: MeshBuffers) -> Result<Mesh, MeshBuildError> {
    buffers.validate()?;

    if let Some(normals) = &mut buffers.normals {
        for normal in normals {
            *normal = normal.as_dvec3().normalize_or_zero().as_vec3();
        }
    }

    let vertex_count = buffers.positions.len();
    let loop_count = buffers.indices.len();
    let uv_count = buffers.uvs.is_some().then_some(loop_count);
    let mut mesh = Mesh {
        topology: Topology::with_capacity(vertex_count, loop_count),
        attributes: Attributes::with_capacity(vertex_count, loop_count, uv_count),
        ..Mesh::default()
    };

    let positions = mesh.attributes.positions_mut();
    let verts: Vec<_> = buffers
        .positions
        .iter()
        .map(|&position| {
            let vert = mesh.topology.verts.insert(Vert { edge: None });
            positions.insert(vert, position);
            vert
        })
        .collect();

    let mut edges = HashMap::new();
    let faces: Vec<_> = buffers
        .face_indices()
        .map(|indices| import_face(&mut mesh, &buffers, indices, &verts, &mut edges))
        .collect();

    add_custom_attributes(&mut mesh, buffers, &verts, &faces, &edges);
    Ok(mesh)
}

/// Moves validated custom attribute buffers into the mesh. Vertex values pair with `verts`, and
/// face values with `faces`. Corner values pair with the corners of `faces` in input order. Edge
/// values pair with `edges`, the edges of each pair of vertex indices.
fn add_custom_attributes(
    mesh: &mut Mesh,
    buffers: MeshBuffers,
    verts: &[VertKey],
    faces: &[FaceKey],
    edges: &HashMap<(u32, u32), EdgeKey>,
) {
    let MeshBuffers {
        vertex_attributes,
        corner_attributes,
        edge_indices,
        edge_attributes,
        face_attributes,
        ..
    } = buffers;

    for (name, values) in vertex_attributes {
        let values = values.into_map(verts.iter().copied());
        mesh.attributes.add(name, values);
    }

    for (name, values) in face_attributes {
        let values = values.into_map(faces.iter().copied());
        mesh.attributes.add(name, values);
    }

    let listed_edges: Vec<_> = edge_indices
        .chunks_exact(2)
        .map(|pair| edges[&edge_pair(pair[0], pair[1])])
        .collect();

    for EdgeAttributeBuffer {
        name,
        values,
        default,
    } in edge_attributes
    {
        let values = values.into_map_with_default(
            listed_edges.iter().copied(),
            default.as_ref(),
            edges.values().copied(),
        );
        mesh.attributes.add(name, values);
    }

    if corner_attributes.is_empty() {
        return;
    }

    let corners: Vec<_> = faces
        .iter()
        .flat_map(|&face| mesh.topology.face_loops(face))
        .collect();

    for (name, values) in corner_attributes {
        let values = values.into_map(corners.iter().copied());
        mesh.attributes.add(name, values);
    }
}

fn import_face(
    mesh: &mut Mesh,
    buffers: &MeshBuffers,
    indices: &[u32],
    verts: &[VertKey],
    edges: &mut HashMap<(u32, u32), EdgeKey>,
) -> FaceKey {
    let corners: Vec<_> = indices
        .iter()
        .enumerate()
        .map(|(corner, &index)| {
            let pair = edge_pair(index, indices[(corner + 1) % indices.len()]);
            let edge = *edges.entry(pair).or_insert_with(|| {
                mesh.topology
                    .insert_edge([verts[pair.0 as usize], verts[pair.1 as usize]])
            });
            (verts[index as usize], edge)
        })
        .collect();
    let face = mesh.topology.insert_face(&corners);

    let mut loop_key = mesh.topology.faces[face].loop_;
    for &index in indices {
        if let Some(normals) = &buffers.normals {
            mesh.attributes
                .normals_mut()
                .insert(loop_key, normals[index as usize]);
        }

        if let Some((uvs, values)) = buffers.uvs.as_ref().zip(mesh.attributes.uvs_mut()) {
            values.insert(loop_key, uvs[index as usize]);
        }
        loop_key = mesh.topology.loops[loop_key].next;
    }

    if buffers.normals.is_some() {
        return face;
    }

    let normal = FaceRef {
        topo: &mesh.topology,
        attrs: &mesh.attributes,
        key: face,
    }
    .normal();

    let normals = mesh.attributes.normals_mut();
    let mut current = mesh.topology.faces[face].loop_;
    for _ in 0..indices.len() {
        normals.insert(current, normal);
        current = mesh.topology.loops[current].next;
    }

    face
}

/// Each attribute's name and values, paired with the number of values it should have.
fn with_length(
    attributes: &[(String, AttributeBuffer)],
    expected: usize,
) -> impl Iterator<Item = (&String, &AttributeBuffer, usize)> {
    attributes
        .iter()
        .map(move |(name, values)| (name, values, expected))
}

/// The vertex indices of an edge, in a canonical order.
fn edge_pair(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}
