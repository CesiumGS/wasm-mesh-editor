use std::collections::HashSet;

use glam::Vec2;
use slotmap::Key;

use crate::ComponentType;

use super::*;

fn triangle_buffers() -> MeshBuffers {
    MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2],
        face_vertex_counts: None,
    }
}

fn assert_connectivity(mesh: &Mesh) {
    let topo = &mesh.topology;
    for (vert_key, vert) in &topo.verts {
        let expected: HashSet<_> = topo
            .edges
            .iter()
            .filter(|(_, edge)| edge.verts.contains(&vert_key))
            .map(|(key, _)| key)
            .collect();
        let mut seen = HashSet::new();
        if let Some(first) = vert.edge {
            let mut current = first;
            loop {
                assert!(seen.insert(current), "disk cycle must close at its entry");
                let edge = &topo.edges[current];
                let side = edge.verts.iter().position(|&key| key == vert_key).unwrap();
                let next_key = edge.disk_next[side];
                let next = &topo.edges[next_key];
                let next_side = next.verts.iter().position(|&key| key == vert_key).unwrap();
                assert_eq!(next.disk_prev[next_side], current);
                current = next_key;
                if current == first {
                    break;
                }
            }
        }
        assert_eq!(seen, expected);
        assert!(mesh.attributes.positions.contains_key(vert_key));
    }

    for (edge_key, edge) in &topo.edges {
        assert_ne!(edge.verts[0], edge.verts[1]);
        let expected: HashSet<_> = topo
            .loops
            .iter()
            .filter(|(_, loop_)| loop_.edge == edge_key)
            .map(|(key, _)| key)
            .collect();
        let first = edge.loop_.unwrap();
        let mut seen = HashSet::new();
        let mut current = first;
        loop {
            assert!(seen.insert(current), "radial cycle must close at its entry");
            let loop_ = &topo.loops[current];
            assert_eq!(loop_.edge, edge_key);
            assert_eq!(topo.loops[loop_.radial_next].radial_prev, current);
            current = loop_.radial_next;
            if current == first {
                break;
            }
        }
        assert_eq!(seen, expected);
    }

    let mut all_loops = HashSet::new();
    for (face_key, face) in &topo.faces {
        let mut current = face.loop_;
        for _ in 0..face.len {
            assert!(
                all_loops.insert(current),
                "each loop belongs to one face cycle"
            );
            let loop_ = &topo.loops[current];
            let next = &topo.loops[loop_.next];
            assert_eq!(loop_.face, face_key);
            assert_eq!(next.prev, current);
            let edge = &topo.edges[loop_.edge];
            assert!(edge.verts.contains(&loop_.vert));
            assert!(edge.verts.contains(&next.vert));
            assert!(mesh.attributes.normals.contains_key(current));
            current = loop_.next;
        }
        assert_eq!(current, face.loop_);
    }
    assert_eq!(all_loops.len(), topo.loops.len());
}

#[test]
fn new_and_default_create_empty_meshes_at_vertex_level() {
    for mesh in [Mesh::new(), Mesh::default()] {
        assert!(mesh.topology.verts.is_empty());
        assert!(mesh.topology.edges.is_empty());
        assert!(mesh.topology.loops.is_empty());
        assert!(mesh.topology.faces.is_empty());
        assert!(mesh.attributes.positions.is_empty());
        assert!(mesh.attributes.normals.is_empty());
        assert!(mesh.attributes.uvs.is_empty());
        assert!(mesh.selection.verts.is_empty());
        assert!(mesh.selection.edges.is_empty());
        assert!(mesh.selection.faces.is_empty());
        assert_eq!(mesh.selection.level, ComponentType::Vertex);
        assert_connectivity(&mesh);
    }
}

#[test]
fn accessors_borrow_mesh_stores() {
    let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    assert!(std::ptr::eq(mesh.topology(), &mesh.topology));
    assert!(std::ptr::eq(mesh.attributes(), &mesh.attributes));

    let view = mesh.selection();
    assert!(std::ptr::eq(view.state, &mesh.selection));
    assert!(std::ptr::eq(view.topo, &mesh.topology));
    assert!(std::ptr::eq(view.attrs, &mesh.attributes));

    let vert_key = mesh.topology.verts.keys().next().unwrap();
    let topology = &mesh.topology as *const Topology;
    let attributes = &mesh.attributes as *const Attributes;
    let state = &mesh.selection as *const SelectionState;
    let selection = mesh.selection_mut();
    assert!(std::ptr::eq(selection.topo, topology));
    assert!(std::ptr::eq(selection.attrs, attributes));
    assert!(std::ptr::eq(&*selection.state, state));
    selection.state.verts.insert(vert_key);
    assert!(mesh.selection().state.verts.contains(&vert_key));
}

#[test]
fn component_accessors_return_views_for_live_keys() {
    let mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    for key in mesh.topology.verts.keys() {
        let view = mesh.vert(key).unwrap();
        assert_eq!(view.key(), key);
        assert!(std::ptr::eq(view.topo, &mesh.topology));
        assert!(std::ptr::eq(view.attrs, &mesh.attributes));
    }
    for key in mesh.topology.edges.keys() {
        let view = mesh.edge(key).unwrap();
        assert_eq!(view.key(), key);
        assert!(std::ptr::eq(view.topo, &mesh.topology));
    }
    for key in mesh.topology.faces.keys() {
        let view = mesh.face(key).unwrap();
        assert_eq!(view.key(), key);
        assert!(std::ptr::eq(view.topo, &mesh.topology));
        assert!(std::ptr::eq(view.attrs, &mesh.attributes));
    }
}

#[test]
fn component_accessors_reject_missing_and_removed_keys() {
    let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    assert!(mesh.vert(VertKey::null()).is_none());
    assert!(mesh.edge(EdgeKey::null()).is_none());
    assert!(mesh.face(FaceKey::null()).is_none());

    let vert_key = mesh.topology.verts.keys().next().unwrap();
    let edge_key = mesh.topology.edges.keys().next().unwrap();
    let face_key = mesh.topology.faces.keys().next().unwrap();
    mesh.topology.verts.remove(vert_key);
    mesh.topology.edges.remove(edge_key);
    mesh.topology.faces.remove(face_key);
    assert!(mesh.vert(vert_key).is_none());
    assert!(mesh.edge(edge_key).is_none());
    assert!(mesh.face(face_key).is_none());
}

#[test]
fn capacity_constructors_reserve_without_populating() {
    for (vertex_capacity, loop_capacity, uv_capacity) in [(0, 0, 0), (5, 13, 0), (7, 19, 19)] {
        let topology = Topology::with_capacity(vertex_capacity, loop_capacity);
        assert!(topology.verts.is_empty());
        assert!(topology.edges.is_empty());
        assert!(topology.loops.is_empty());
        assert!(topology.faces.is_empty());
        assert!(topology.verts.capacity() >= vertex_capacity);
        assert!(topology.loops.capacity() >= loop_capacity);

        let attributes = Attributes::with_capacity(vertex_capacity, loop_capacity, uv_capacity);
        assert!(attributes.positions.is_empty());
        assert!(attributes.normals.is_empty());
        assert!(attributes.uvs.is_empty());
        assert!(attributes.positions.capacity() >= vertex_capacity);
        assert!(attributes.normals.capacity() >= loop_capacity);
        assert!(attributes.uvs.capacity() >= uv_capacity);
        if uv_capacity == 0 {
            assert_eq!(attributes.uvs.capacity(), 0);
        }
    }
}

#[test]
fn builds_triangle_cycles_positions_and_flat_normals() {
    let options = triangle_buffers();
    let positions = options.positions.clone();
    let mesh = Mesh::from_buffers(options).unwrap();
    assert_eq!(mesh.topology.verts.len(), 3);
    assert_eq!(mesh.topology.edges.len(), 3);
    assert_eq!(mesh.topology.loops.len(), 3);
    assert_eq!(mesh.topology.faces.len(), 1);
    let face = mesh.topology.faces.values().next().unwrap();
    let mut current = face.loop_;
    for position in positions {
        let loop_ = &mesh.topology.loops[current];
        assert_eq!(mesh.attributes.positions[loop_.vert], position);
        assert_eq!(mesh.attributes.normals[current], Vec3::Z);
        current = loop_.next;
    }
    assert!(mesh.attributes.uvs.is_empty());
    assert!(mesh.selection.verts.is_empty());
    assert!(mesh.selection.edges.is_empty());
    assert!(mesh.selection.faces.is_empty());
    assert_eq!(mesh.selection.level, ComponentType::Vertex);
    assert_connectivity(&mesh);
}

#[test]
fn builds_shared_edges_with_any_winding_and_radial_valence() {
    for indices in [
        vec![0, 1, 2, 1, 0, 3],
        vec![0, 1, 2, 0, 1, 3],
        vec![0, 1, 2, 1, 0, 3, 0, 1, 4],
    ] {
        let mut options = triangle_buffers();
        options.positions.extend([Vec3::Z, -Vec3::Y]);
        let face_count = indices.len() / 3;
        options.indices = indices;
        let mesh = Mesh::from_buffers(options).unwrap();
        assert_eq!(mesh.topology.edges.len(), 1 + 2 * face_count);
        assert_eq!(mesh.topology.faces.len(), face_count);
        assert_eq!(mesh.topology.loops.len(), 3 * face_count);
        let verts: Vec<_> = mesh.topology.verts.keys().collect();
        let shared_edge = mesh
            .topology
            .edges
            .iter()
            .find(|(_, edge)| edge.verts == [verts[0], verts[1]])
            .map(|(key, _)| key)
            .unwrap();
        assert_eq!(
            mesh.topology
                .loops
                .values()
                .filter(|loop_| loop_.edge == shared_edge)
                .count(),
            face_count
        );
        assert_connectivity(&mesh);
    }
}

#[test]
fn generates_distinct_flat_normals_across_a_shared_edge() {
    let mut options = triangle_buffers();
    options.positions.push(Vec3::Z);
    options.indices = vec![0, 1, 2, 0, 1, 3];
    let mesh = Mesh::from_buffers(options).unwrap();
    for (face_key, expected) in mesh.topology.faces.keys().zip([Vec3::Z, -Vec3::Y]) {
        assert_eq!(mesh.face(face_key).unwrap().normal(), expected);
        for (key, loop_) in &mesh.topology.loops {
            if loop_.face == face_key {
                assert_eq!(mesh.attributes.normals[key], expected);
            }
        }
    }
    assert_connectivity(&mesh);
}

#[test]
fn builds_mixed_polygons_in_input_order() {
    let options = MeshBuffers {
        positions: vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 3, 4, 5, 0, 2, 5, 0, 2, 3, 5],
        face_vertex_counts: Some(vec![6, 3, 4]),
    };
    let expected_faces = [vec![0, 1, 2, 3, 4, 5], vec![0, 2, 5], vec![0, 2, 3, 5]];
    let mesh = Mesh::from_buffers(options).unwrap();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    assert_eq!(mesh.topology.faces.len(), 3);
    assert_eq!(mesh.topology.loops.len(), 13);
    assert_eq!(mesh.topology.edges.len(), 9);
    for ((face_key, face), indices) in mesh.topology.faces.iter().zip(expected_faces) {
        assert_eq!(face.len as usize, indices.len());
        let view = mesh.face(face_key).unwrap();
        assert_eq!(view.key(), face_key);
        assert_eq!(view.normal(), Vec3::Z);
        let expected_verts: Vec<_> = indices.iter().map(|&index| verts[index as usize]).collect();
        assert_eq!(view.verts().collect::<Vec<_>>(), expected_verts);
        let mut current = face.loop_;
        for index in indices {
            let loop_ = &mesh.topology.loops[current];
            assert_eq!(loop_.vert, verts[index as usize]);
            assert_eq!(mesh.attributes.normals[current], Vec3::Z);
            current = loop_.next;
        }
    }
    assert_connectivity(&mesh);
}

#[test]
fn face_normals_follow_position_edits_without_overwriting_shading_normals() {
    let mut options = triangle_buffers();
    options.positions.push(Vec3::Z);
    options.indices = vec![0, 1, 2, 1, 0, 3];
    options.normals = Some(vec![Vec3::X; 4]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let edited_vertex = mesh.topology.verts.keys().nth(2).unwrap();
    assert_eq!(mesh.face(faces[0]).unwrap().normal(), Vec3::Z);
    assert_eq!(mesh.face(faces[1]).unwrap().normal(), Vec3::Y);

    mesh.attributes.positions[edited_vertex] = Vec3::Z;

    assert_eq!(mesh.face(faces[0]).unwrap().normal(), -Vec3::Y);
    assert_eq!(mesh.face(faces[1]).unwrap().normal(), Vec3::Y);
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::X)
    );
    assert!(mesh.face(FaceKey::null()).is_none());
    mesh.topology.faces.remove(faces[0]);
    assert!(mesh.face(faces[0]).is_none());
}

#[test]
fn copies_normalized_vertex_attributes_to_every_referencing_loop() {
    let mut options = triangle_buffers();
    options.positions.push(Vec3::Z);
    options.indices = vec![2, 0, 1, 1, 0, 3];
    let normals = vec![2.0 * Vec3::X, Vec3::Y, Vec3::ZERO, -Vec3::Z];
    let expected_normals = [Vec3::X, Vec3::Y, Vec3::ZERO, -Vec3::Z];
    let uvs = vec![Vec2::X, Vec2::Y, Vec2::ONE, Vec2::ZERO];
    options.normals = Some(normals);
    options.uvs = Some(uvs.clone());
    let mesh = Mesh::from_buffers(options).unwrap();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    assert_eq!(mesh.attributes.normals.len(), 6);
    assert_eq!(mesh.attributes.uvs.len(), 6);
    for (key, loop_) in &mesh.topology.loops {
        let index = verts.iter().position(|&vert| vert == loop_.vert).unwrap();
        assert_eq!(mesh.attributes.normals[key], expected_normals[index]);
        assert_eq!(mesh.attributes.uvs[key], uvs[index]);
    }
    assert_connectivity(&mesh);
}

#[test]
fn normalizes_supplied_normals_across_finite_scales() {
    for scale in [
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        1.0e-30,
        1.0,
        1.0e30,
        f32::MAX,
    ] {
        let mut options = triangle_buffers();
        options.normals = Some(vec![Vec3::new(scale, -scale, scale), Vec3::ZERO, -Vec3::Z]);
        let expected = [Vec3::new(1.0, -1.0, 1.0).normalize(), Vec3::ZERO, -Vec3::Z];
        assert_eq!(options.validate(), Ok(()));
        let mesh = Mesh::from_buffers(options).unwrap();
        let verts: Vec<_> = mesh.topology.verts.keys().collect();
        for (key, loop_) in &mesh.topology.loops {
            let index = verts.iter().position(|&vert| vert == loop_.vert).unwrap();
            let normal = mesh.attributes.normals[key];
            assert!(normal.is_finite());
            assert!(normal.abs_diff_eq(expected[index], 1.0e-6));
        }
        assert_connectivity(&mesh);
    }
}

#[test]
fn rejects_non_finite_normals_including_unused_vertices() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for component in 0..3 {
            for vertex in [1, 3] {
                let mut options = triangle_buffers();
                options.positions.push(Vec3::ONE);
                let mut normals = vec![Vec3::Z; 4];
                normals[vertex][component] = invalid;
                options.normals = Some(normals);
                let expected = MeshBuildError::NonFiniteNormal { vertex };
                assert_eq!(options.validate(), Err(expected.clone()));
                assert_eq!(Mesh::from_buffers(options).err(), Some(expected));
            }
        }
    }
}

#[test]
fn preserves_unwelded_vertices_and_isolated_positions() {
    let mut options = triangle_buffers();
    options
        .positions
        .extend([Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z]);
    options.indices = vec![0, 1, 2, 3, 4, 5];
    options.uvs = Some(vec![
        Vec2::ZERO,
        Vec2::X,
        Vec2::Y,
        Vec2::ONE,
        Vec2::ONE,
        Vec2::ONE,
        Vec2::ZERO,
    ]);
    let mesh = Mesh::from_buffers(options).unwrap();
    assert_eq!(mesh.topology.verts.len(), 7);
    assert_eq!(mesh.attributes.positions.len(), 7);
    assert_eq!(mesh.topology.edges.len(), 6);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    assert_eq!(
        mesh.attributes.positions[verts[0]],
        mesh.attributes.positions[verts[3]]
    );
    assert_ne!(verts[0], verts[3]);
    assert!(mesh.topology.verts[verts[6]].edge.is_none());
    let loops: Vec<_> = mesh
        .topology
        .faces
        .values()
        .map(|face| face.loop_)
        .collect();
    assert_eq!(mesh.attributes.uvs[loops[0]], Vec2::ZERO);
    assert_eq!(mesh.attributes.uvs[loops[1]], Vec2::ONE);
    assert_connectivity(&mesh);
}

#[test]
fn builds_empty_meshes_and_point_clouds() {
    for positions in [vec![], vec![Vec3::ZERO, Vec3::ZERO, Vec3::X]] {
        for face_vertex_counts in [None, Some(vec![])] {
            let mesh = Mesh::from_buffers(MeshBuffers {
                positions: positions.clone(),
                normals: None,
                uvs: None,
                indices: vec![],
                face_vertex_counts,
            })
            .unwrap();
            assert_eq!(mesh.topology.verts.len(), positions.len());
            assert!(mesh.topology.edges.is_empty());
            assert!(mesh.topology.loops.is_empty());
            assert!(mesh.topology.faces.is_empty());
            assert!(mesh.attributes.normals.is_empty());
            assert!(mesh.attributes.uvs.is_empty());
            assert_connectivity(&mesh);
        }
    }
}

#[test]
fn flat_normals_handle_degeneracy_scale_and_reversed_winding() {
    for positions in [
        vec![Vec3::ZERO; 3],
        vec![Vec3::ZERO, Vec3::X, 2.0 * Vec3::X],
    ] {
        let mut options = triangle_buffers();
        options.positions = positions;
        let mesh = Mesh::from_buffers(options).unwrap();
        for key in mesh.topology.faces.keys() {
            assert_eq!(mesh.face(key).unwrap().normal(), Vec3::ZERO);
        }
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == Vec3::ZERO)
        );
        assert_connectivity(&mesh);
    }
    for scale in [1.0e-30, 1.0, 1.0e30] {
        let mut options = triangle_buffers();
        options.positions = vec![Vec3::ZERO, scale * Vec3::X, scale * Vec3::Y];
        options.indices.reverse();
        let mesh = Mesh::from_buffers(options).unwrap();
        for key in mesh.topology.faces.keys() {
            assert_eq!(mesh.face(key).unwrap().normal(), -Vec3::Z);
        }
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == -Vec3::Z)
        );
    }
}

#[test]
fn constructor_returns_validation_errors() {
    let mut options = triangle_buffers();
    options.indices = vec![0, 1, 0];
    assert!(matches!(
        Mesh::from_buffers(options),
        Err(MeshBuildError::RepeatedFaceVertex { face: 0, index: 0 })
    ));
}

#[test]
fn validate_accepts_coincident_positions_and_empty_meshes() {
    let mut options = triangle_buffers();
    options.positions[1] = options.positions[0];
    assert_eq!(options.validate(), Ok(()));
    options.indices.clear();
    assert_eq!(options.validate(), Ok(()));
    options.positions.clear();
    options.face_vertex_counts = Some(vec![]);
    assert_eq!(options.validate(), Ok(()));
}

#[test]
fn validate_attribute_lengths() {
    let mut options = triangle_buffers();
    options.normals = Some(vec![Vec3::Z]);
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::AttributeLengthMismatch {
            attribute: "normals",
            expected: 3,
            actual: 1,
        })
    );
    options.normals = Some(vec![Vec3::Z; 3]);
    options.uvs = Some(vec![Vec2::ZERO; 2]);
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::AttributeLengthMismatch {
            attribute: "uvs",
            expected: 3,
            actual: 2,
        })
    );
    options.uvs = Some(vec![Vec2::ZERO; 3]);
    assert_eq!(options.validate(), Ok(()));
}

#[test]
fn validate_triangle_and_polygon_counts() {
    let mut options = triangle_buffers();
    options.indices.pop();
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::IncompleteTriangle { index_count: 2 })
    );
    options.face_vertex_counts = Some(vec![2]);
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::FaceTooSmall {
            face: 0,
            vertex_count: 2
        })
    );
    options.face_vertex_counts = Some(vec![u32::MAX]);
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::FaceVertexCountMismatch)
    );
    options.face_vertex_counts = Some(vec![]);
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::FaceVertexCountMismatch)
    );
    options = triangle_buffers();
    options.face_vertex_counts = Some(vec![3]);
    assert_eq!(options.validate(), Ok(()));
}

#[test]
fn validate_index_bounds_and_repeated_face_indices() {
    let mut options = triangle_buffers();
    options.indices = vec![0, 1, u32::MAX];
    assert_eq!(
        options.validate(),
        Err(MeshBuildError::VertexIndexOutOfBounds {
            corner: 2,
            index: u32::MAX,
            vertex_count: 3,
        })
    );
    for indices in [vec![0, 0, 2], vec![0, 1, 0], vec![0, 1, 2, 1]] {
        options.face_vertex_counts = Some(vec![indices.len() as u32]);
        options.indices = indices;
        assert!(matches!(
            options.validate(),
            Err(MeshBuildError::RepeatedFaceVertex { face: 0, .. })
        ));
    }
    options.indices = vec![0, 1, 2, 2, 1, 0];
    options.face_vertex_counts = Some(vec![3, 3]);
    assert_eq!(options.validate(), Ok(()));
}
