//! Validated buffer import into a newly constructed mesh.

use std::collections::HashMap;
use std::fmt;

use glam::{Vec2, Vec3};

use super::Mesh;
use crate::selection::SelectionState;
use crate::{Attributes, EdgeKey, FaceRef, Topology, Vert, VertKey};

/// Decoded vertex attribute and face index buffers describing a mesh to build.
/// File-format accessors, byte strides, and component types must be resolved first.
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
        }
    }
}

impl std::error::Error for MeshBuildError {}

impl MeshBuffers {
    /// Checks buffer lengths, finite normals, face sizes, index bounds, and repeated indices per face.
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
        Ok(())
    }

    fn face_indices(&self) -> impl Iterator<Item = &[u32]> {
        let face_count = self
            .face_vertex_counts
            .as_ref()
            .map_or(self.indices.len() / 3, Vec::len);
        (0..face_count).scan(0, |offset, face| {
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
    let uv_count = if buffers.uvs.is_some() { loop_count } else { 0 };
    let mut mesh = Mesh {
        topology: Topology::with_capacity(vertex_count, loop_count),
        attributes: Attributes::with_capacity(vertex_count, loop_count, uv_count),
        selection: SelectionState::default(),
        changes: Default::default(),
        face_normals: Default::default(),
    };

    let verts: Vec<_> = buffers
        .positions
        .iter()
        .map(|&position| {
            let vert = mesh.topology.verts.insert(Vert { edge: None });
            mesh.attributes.positions.insert(vert, position);
            vert
        })
        .collect();

    let mut edges = HashMap::new();
    for indices in buffers.face_indices() {
        import_face(&mut mesh, &buffers, indices, &verts, &mut edges);
    }
    Ok(mesh)
}

fn import_face(
    mesh: &mut Mesh,
    buffers: &MeshBuffers,
    indices: &[u32],
    verts: &[VertKey],
    edges: &mut HashMap<(u32, u32), EdgeKey>,
) {
    let corners: Vec<_> = indices
        .iter()
        .enumerate()
        .map(|(corner, &index)| {
            let next_index = indices[(corner + 1) % indices.len()];
            let pair = (index.min(next_index), index.max(next_index));
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
                .normals
                .insert(loop_key, normals[index as usize]);
        }

        if let Some(uvs) = &buffers.uvs {
            mesh.attributes.uvs.insert(loop_key, uvs[index as usize]);
        }
        loop_key = mesh.topology.loops[loop_key].next;
    }

    if buffers.normals.is_some() {
        return;
    }

    let normal = FaceRef {
        topo: &mesh.topology,
        attrs: &mesh.attributes,
        key: face,
    }
    .normal();

    let mut current = mesh.topology.faces[face].loop_;
    for _ in 0..indices.len() {
        mesh.attributes.normals.insert(current, normal);
        current = mesh.topology.loops[current].next;
    }
}
