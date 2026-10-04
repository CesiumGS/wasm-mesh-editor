use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use glam::Vec2;
use slotmap::Key;

use crate::{ComponentTypes, SelectionChange};

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
        assert_eq!(topo.vert_edges(vert_key).count(), expected.len());
        assert_eq!(topo.vert_edges(vert_key).collect::<HashSet<_>>(), expected);
        let expected_neighbors: HashSet<_> = expected
            .iter()
            .flat_map(|&key| topo.edges[key].verts)
            .filter(|&key| key != vert_key)
            .collect();
        assert_eq!(
            topo.vert_neighbors(vert_key).collect::<HashSet<_>>(),
            expected_neighbors
        );
        assert!(mesh.attributes.positions.contains_key(vert_key));

        let expected_loops: HashSet<_> = topo
            .loops
            .iter()
            .filter(|(_, corner)| corner.vert == vert_key)
            .map(|(key, _)| key)
            .collect();
        assert_eq!(topo.vert_loops(vert_key).count(), expected_loops.len());
        assert_eq!(
            topo.vert_loops(vert_key).collect::<HashSet<_>>(),
            expected_loops
        );
        let expected_faces: HashSet<_> = topo
            .loops
            .values()
            .filter(|loop_| loop_.vert == vert_key)
            .map(|loop_| loop_.face)
            .collect();
        let vertex = mesh.vert(vert_key).unwrap();
        assert_eq!(
            vertex.edges().collect::<Vec<_>>(),
            topo.vert_edges(vert_key).collect::<Vec<_>>()
        );
        assert_eq!(
            vertex.neighbors().collect::<Vec<_>>(),
            topo.vert_neighbors(vert_key).collect::<Vec<_>>()
        );
        assert_eq!(
            vertex.loops().collect::<Vec<_>>(),
            topo.vert_loops(vert_key).collect::<Vec<_>>()
        );
        assert_eq!(
            topo.vert_faces(vert_key).collect::<HashSet<_>>(),
            expected_faces
        );
        assert_eq!(vertex.faces().count(), expected_faces.len());
        assert_eq!(vertex.faces().collect::<HashSet<_>>(), expected_faces);
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
        let view = mesh.edge(edge_key).unwrap();
        assert_eq!(topo.edge_verts(edge_key), edge.verts);
        assert_eq!(view.verts(), topo.edge_verts(edge_key));
        assert_eq!(topo.edge_loops(edge_key).count(), expected.len());
        assert_eq!(topo.edge_loops(edge_key).collect::<HashSet<_>>(), expected);
        assert_eq!(view.loops().count(), expected.len());
        assert_eq!(view.loops().collect::<HashSet<_>>(), expected);
        let expected_faces: HashSet<_> = expected.iter().map(|&key| topo.loops[key].face).collect();
        assert_eq!(
            topo.edge_faces(edge_key).collect::<HashSet<_>>(),
            expected_faces
        );
        assert_eq!(view.faces().count(), expected_faces.len());
        assert_eq!(view.faces().collect::<HashSet<_>>(), expected_faces);
    }

    let mut all_loops = HashSet::new();
    for (face_key, face) in &topo.faces {
        let mut current = face.loop_;
        let mut expected_loops = Vec::new();
        for _ in 0..face.len {
            expected_loops.push(current);
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
        let view = mesh.face(face_key).unwrap();
        assert_eq!(
            topo.face_loops(face_key).collect::<Vec<_>>(),
            expected_loops
        );
        assert_eq!(view.loops().collect::<Vec<_>>(), expected_loops);
        let expected_verts: Vec<_> = expected_loops
            .iter()
            .map(|&key| topo.loops[key].vert)
            .collect();
        let expected_edges: Vec<_> = expected_loops
            .iter()
            .map(|&key| topo.loops[key].edge)
            .collect();
        assert_eq!(
            topo.face_verts(face_key).collect::<Vec<_>>(),
            expected_verts
        );
        assert_eq!(view.verts().collect::<Vec<_>>(), expected_verts);
        assert_eq!(
            topo.face_edges(face_key).collect::<Vec<_>>(),
            expected_edges
        );
        assert_eq!(view.edges().collect::<Vec<_>>(), expected_edges);
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
        assert_eq!(mesh.selection.level, ComponentTypes::VERTEX);
        assert_connectivity(&mesh);
    }
}

#[test]
fn change_subscriptions_use_mesh_owned_emitter() {
    for mut mesh in [
        Mesh::new(),
        Mesh::default(),
        Mesh::from_buffers(triangle_buffers()).unwrap(),
    ] {
        let received = Rc::new(RefCell::new(0));
        let subscription = mesh.changes().subscribe(&received, |received, change| {
            assert!(matches!(change, MeshChange::Selection(_)));
            *received += 1;
        });
        let change = MeshChange::Selection(SelectionChange::default());

        mesh.changes.emit(&change);
        assert_eq!(*received.borrow(), 1);
        drop(subscription);
        mesh.changes.emit(&change);
        assert_eq!(*received.borrow(), 1);

        let subscription = mesh.changes().subscribe(&received, |received, _| {
            *received += 1;
        });
        drop(mesh);
        drop(subscription);
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
        assert!(mesh.topology().contains(key));
        let Some(ComponentRef::Vert(component)) = mesh.component(key) else {
            panic!("expected a vertex view");
        };
        assert_eq!(component.key(), key);
        let view = mesh.vert(key).unwrap();
        assert_eq!(view.key(), key);
        assert!(std::ptr::eq(view.topo, &mesh.topology));
        assert!(std::ptr::eq(view.attrs, &mesh.attributes));
    }
    for key in mesh.topology.edges.keys() {
        assert!(mesh.topology().contains(key));
        let Some(ComponentRef::Edge(component)) = mesh.component(ComponentKey::Edge(key)) else {
            panic!("expected an edge view");
        };
        assert_eq!(component.key(), key);
        let view = mesh.edge(key).unwrap();
        assert_eq!(view.key(), key);
        assert!(std::ptr::eq(view.topo, &mesh.topology));
    }
    for key in mesh.topology.faces.keys() {
        assert!(mesh.topology().contains(ComponentKey::Face(key)));
        let Some(ComponentRef::Face(component)) = mesh.component(key) else {
            panic!("expected a face view");
        };
        assert_eq!(component.key(), key);
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
    for key in [
        ComponentKey::Vert(VertKey::null()),
        ComponentKey::Edge(EdgeKey::null()),
        ComponentKey::Face(FaceKey::null()),
    ] {
        assert!(!mesh.topology().contains(key));
        assert!(mesh.component(key).is_none());
    }

    let vert_key = mesh.topology.verts.keys().next().unwrap();
    let edge_key = mesh.topology.edges.keys().next().unwrap();
    let face_key = mesh.topology.faces.keys().next().unwrap();
    mesh.topology.verts.remove(vert_key);
    mesh.topology.edges.remove(edge_key);
    mesh.topology.faces.remove(face_key);
    assert!(mesh.vert(vert_key).is_none());
    assert!(mesh.edge(edge_key).is_none());
    assert!(mesh.face(face_key).is_none());
    for key in [
        ComponentKey::Vert(vert_key),
        ComponentKey::Edge(edge_key),
        ComponentKey::Face(face_key),
    ] {
        assert!(!mesh.topology().contains(key));
        assert!(mesh.component(key).is_none());
    }
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
    assert_eq!(mesh.selection.level, ComponentTypes::VERTEX);
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
fn vertex_faces_cover_disconnected_fans_and_wire_edges() {
    let mut options = triangle_buffers();
    options
        .positions
        .extend([Vec3::Z, -Vec3::Y, -Vec3::X, Vec3::ONE]);
    options.indices = vec![0, 1, 2, 0, 3, 4];
    let mut mesh = Mesh::from_buffers(options).unwrap();
    assert_connectivity(&mesh);

    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let wire = mesh.topology.insert_edge([verts[0], verts[5]]);
    let vertex = mesh.vert(verts[0]).unwrap();
    assert_eq!(vertex.faces().count(), 2);
    assert_eq!(
        vertex.faces().collect::<HashSet<_>>(),
        mesh.topology.faces.keys().collect()
    );
    assert_eq!(mesh.vert(verts[5]).unwrap().faces().count(), 0);
    assert_eq!(mesh.vert(verts[6]).unwrap().faces().count(), 0);
    assert_eq!(mesh.edge(wire).unwrap().loops().count(), 0);
    assert_eq!(mesh.edge(wire).unwrap().faces().count(), 0);
    assert_eq!(mesh.topology.edge_loops(wire).count(), 0);
    assert_eq!(mesh.topology.edge_faces(wire).count(), 0);
    assert_eq!(mesh.topology.vert_loops(verts[0]).count(), 2);
    assert_eq!(mesh.topology.vert_edges(verts[0]).count(), 5);
    assert_eq!(
        mesh.topology
            .vert_neighbors(verts[0])
            .collect::<HashSet<_>>(),
        verts[1..6].iter().copied().collect()
    );
    assert_eq!(
        mesh.topology.vert_edges(verts[5]).collect::<Vec<_>>(),
        vec![wire]
    );
    assert_eq!(
        mesh.topology.vert_neighbors(verts[5]).collect::<Vec<_>>(),
        vec![verts[0]]
    );
    assert_eq!(mesh.topology.vert_loops(verts[5]).count(), 0);
    assert_eq!(mesh.topology.vert_edges(verts[6]).count(), 0);
    assert_eq!(mesh.topology.vert_neighbors(verts[6]).count(), 0);
    assert_eq!(mesh.topology.vert_loops(verts[6]).count(), 0);
}

#[test]
fn topology_traversal_allows_attribute_writes() {
    let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    let Mesh {
        topology,
        attributes,
        ..
    } = &mut mesh;
    let face = topology.faces.keys().next().unwrap();

    for vertex in topology.face_verts(face) {
        attributes.positions[vertex] += Vec3::Z;
        for neighbor in topology.vert_neighbors(vertex) {
            for corner in topology.vert_loops(neighbor) {
                attributes.normals.insert(corner, Vec3::X);
            }
        }
    }
    for edge in topology.face_edges(face) {
        for corner in topology.edge_loops(edge) {
            attributes.normals.insert(corner, Vec3::Y);
        }
    }
    for corner in topology.face_loops(face) {
        assert_eq!(attributes.normals[corner], Vec3::Y);
        assert_eq!(attributes.positions[topology.loops[corner].vert].z, 1.0);
    }
    assert_eq!(mesh.face(face).unwrap().normal(), Vec3::Z);
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
fn translate_selected_updates_only_moved_positions_and_mixed_face_normals() {
    let mut options = triangle_buffers();
    options.positions.extend([Vec3::Z, -Vec3::X, -Vec3::Y]);
    options.indices = vec![0, 1, 2, 1, 0, 3, 3, 4, 5];
    options.normals = Some(vec![Vec3::X; 6]);
    options.uvs = Some(vec![Vec2::ONE; 6]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let vertices: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    mesh.selection_mut().select(&[faces[0]]);
    let selected: HashSet<_> = mesh.selection().selected().collect();
    let positions = mesh.attributes.positions.clone();
    assert_eq!(
        mesh.selection().boundary().mixed_faces(),
        &HashSet::from([faces[1]])
    );
    let cached_faces = mesh.selection().boundary().mixed_faces() as *const HashSet<FaceKey>;

    for (step, expected_normal) in [(1.0, Vec3::Z), (2.0, Vec3::new(0.0, -1.0, 2.0).normalize())] {
        mesh.translate_selected(Vec3::Y + Vec3::Z);
        for (index, &key) in vertices.iter().enumerate() {
            let expected = positions[key]
                + if index < 3 {
                    step * (Vec3::Y + Vec3::Z)
                } else {
                    Vec3::ZERO
                };
            assert_eq!(mesh.attributes.positions[key], expected);
        }
        for (key, corner) in &mesh.topology.loops {
            let expected = if corner.face == faces[1] {
                expected_normal
            } else {
                Vec3::X
            };
            assert!(mesh.attributes.normals[key].abs_diff_eq(expected, 1.0e-6));
        }
        assert!(std::ptr::eq(
            cached_faces,
            mesh.selection().boundary().mixed_faces()
        ));
        assert_eq!(
            mesh.selection().selected().collect::<HashSet<_>>(),
            selected
        );
        assert_eq!(mesh.selection().level(), ComponentTypes::FACE);
        assert!(mesh.attributes.uvs.values().all(|&uv| uv == Vec2::ONE));
        assert!(mesh.face_normals.is_empty());
        assert_connectivity(&mesh);
    }
}

#[test]
fn translate_selected_updates_mixed_faces_without_selected_faces() {
    for (level, expected_normal) in [
        (ComponentTypes::VERTEX, Vec3::ONE.normalize()),
        (ComponentTypes::EDGE, (Vec3::Y + Vec3::Z).normalize()),
    ] {
        let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
        let vertices: Vec<_> = mesh.topology.verts.keys().collect();
        let face = mesh.topology.faces.keys().next().unwrap();
        mesh.selection_mut().set_level(level);
        if level == ComponentTypes::VERTEX {
            mesh.selection_mut().select(&[vertices[0]]);
        } else {
            let edge = mesh.topology.face_edges(face).next().unwrap();
            mesh.selection_mut().select(&[edge]);
        }
        assert_eq!(mesh.selection().faces().count(), 0);
        let selected: HashSet<_> = mesh.selection().verts().collect();
        let positions = mesh.attributes.positions.clone();

        mesh.translate_selected(Vec3::Z);

        for &key in &vertices {
            let expected = positions[key]
                + if selected.contains(&key) {
                    Vec3::Z
                } else {
                    Vec3::ZERO
                };
            assert_eq!(mesh.attributes.positions[key], expected);
        }
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal.abs_diff_eq(expected_normal, 1.0e-6))
        );
        assert_connectivity(&mesh);
    }
}

#[test]
fn translate_selected_preserves_normals_for_noops_and_rigid_faces() {
    for (selected_count, delta) in [(0, Vec3::Z), (1, Vec3::ZERO), (3, Vec3::Z)] {
        let mut options = triangle_buffers();
        options.normals = Some(vec![Vec3::X; 3]);
        let mut mesh = Mesh::from_buffers(options).unwrap();
        let vertices: Vec<_> = mesh.topology.verts.keys().collect();
        mesh.selection_mut().select(&vertices[..selected_count]);
        let positions = mesh.attributes.positions.clone();

        mesh.translate_selected(delta);

        for (index, &key) in vertices.iter().enumerate() {
            let expected = positions[key]
                + if index < selected_count {
                    delta
                } else {
                    Vec3::ZERO
                };
            assert_eq!(mesh.attributes.positions[key], expected);
        }
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == Vec3::X)
        );
        assert_connectivity(&mesh);
    }

    let mut empty = Mesh::new();
    empty.translate_selected(Vec3::ONE);
    assert!(empty.attributes.positions.is_empty());
    assert!(empty.attributes.normals.is_empty());
}

#[test]
fn translate_selected_moves_wire_and_isolated_vertices() {
    let mut options = triangle_buffers();
    options
        .positions
        .extend([Vec3::Z, 2.0 * Vec3::Z, 3.0 * Vec3::Z]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let vertices: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.topology.insert_edge([vertices[3], vertices[4]]);
    mesh.selection_mut().select(&[vertices[3], vertices[5]]);
    let positions = mesh.attributes.positions.clone();
    let normals = mesh.attributes.normals.clone();

    mesh.translate_selected(Vec3::ONE);

    for (index, &key) in vertices.iter().enumerate() {
        let expected = positions[key]
            + if index == 3 || index == 5 {
                Vec3::ONE
            } else {
                Vec3::ZERO
            };
        assert_eq!(mesh.attributes.positions[key], expected);
    }
    assert_eq!(mesh.attributes.normals.len(), normals.len());
    assert!(
        mesh.attributes
            .normals
            .iter()
            .all(|(key, &normal)| normal == normals[key])
    );
    assert!(mesh.selection().boundary().mixed_faces().is_empty());
}

#[test]
fn translate_selected_updates_normals_when_faces_collapse_and_recover() {
    let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    let vertex = mesh.topology.verts.keys().nth(2).unwrap();
    mesh.selection_mut().select(&[vertex]);

    mesh.translate_selected(-Vec3::Y);
    assert_eq!(mesh.attributes.positions[vertex], Vec3::ZERO);
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::ZERO)
    );

    mesh.translate_selected(Vec3::Y);
    assert_eq!(mesh.attributes.positions[vertex], Vec3::Y);
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::Z)
    );
    assert_connectivity(&mesh);
}

#[test]
fn recompute_flat_normals_updates_only_requested_faces() {
    let mut options = triangle_buffers();
    options.positions.push(Vec3::Z);
    options.indices = vec![0, 1, 2, 1, 0, 3];
    options.normals = Some(vec![Vec3::X; 4]);
    options.uvs = Some(vec![Vec2::ONE; 4]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let edited_vertex = mesh.topology.verts.keys().nth(2).unwrap();
    mesh.attributes.positions[edited_vertex] = Vec3::Z;
    mesh.selection_mut().select(&[edited_vertex]);
    let positions: Vec<_> = mesh
        .attributes
        .positions
        .iter()
        .map(|(key, &value)| (key, value))
        .collect();

    Mesh::recompute_flat_normals(&mesh.topology, &mut mesh.attributes, []);
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::X)
    );
    Mesh::recompute_flat_normals(&mesh.topology, &mut mesh.attributes, [faces[0], faces[0]]);
    for (key, loop_) in &mesh.topology.loops {
        let expected = if loop_.face == faces[0] {
            -Vec3::Y
        } else {
            Vec3::X
        };
        assert_eq!(mesh.attributes.normals[key], expected);
    }
    assert_eq!(
        mesh.selection().verts().collect::<Vec<_>>(),
        vec![edited_vertex]
    );
    assert!(mesh.attributes.uvs.values().all(|&uv| uv == Vec2::ONE));
    for (key, position) in positions {
        assert_eq!(mesh.attributes.positions[key], position);
    }
    assert_connectivity(&mesh);
}

#[test]
fn recompute_smooth_normals_weights_all_incident_faces_by_corner_angle() {
    let mut options = triangle_buffers();
    options.positions = vec![
        Vec3::ZERO,
        2.0 * Vec3::X,
        Vec3::new(1.0, 1.0, 0.0),
        100.0 * Vec3::Z,
    ];
    options.indices = vec![0, 1, 2, 1, 0, 3];
    options.normals = Some(vec![Vec3::X; 4]);
    options.uvs = Some(vec![Vec2::ONE; 4]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let vertex = mesh.topology.verts.keys().next().unwrap();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    mesh.selection_mut().select(&[face]);
    let positions: Vec<_> = mesh
        .attributes
        .positions
        .iter()
        .map(|(key, &value)| (key, value))
        .collect();
    let selected: HashSet<_> = mesh.selection().selected().collect();

    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        [],
    );
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::X)
    );
    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        [vertex, vertex],
    );
    for (key, corner) in &mesh.topology.loops {
        let expected = if corner.vert == vertex {
            Vec3::new(0.0, 2.0, 1.0).normalize()
        } else {
            Vec3::X
        };
        assert!(mesh.attributes.normals[key].abs_diff_eq(expected, 1.0e-6));
    }
    assert_eq!(
        mesh.selection().selected().collect::<HashSet<_>>(),
        selected
    );
    assert!(mesh.attributes.uvs.values().all(|&uv| uv == Vec2::ONE));
    for (key, position) in positions {
        assert_eq!(mesh.attributes.positions[key], position);
    }

    let edited_vertex = mesh.topology.verts.keys().nth(2).unwrap();
    mesh.attributes.positions[edited_vertex] = Vec3::Y;
    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        [vertex],
    );
    for key in mesh.vert(vertex).unwrap().loops() {
        assert!(mesh.attributes.normals[key].abs_diff_eq((Vec3::Y + Vec3::Z).normalize(), 1.0e-6));
    }
    assert_connectivity(&mesh);
}

#[test]
fn recompute_selected_normals_only_update_selected_components() {
    let mut options = triangle_buffers();
    options.positions.push(Vec3::Z);
    options.indices = vec![0, 1, 2, 1, 0, 3];
    options.normals = Some(vec![Vec3::X; 4]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let face = mesh.topology.faces.keys().next().unwrap();
    let vertex = mesh.topology.verts.keys().next().unwrap();

    mesh.shade_flat();
    mesh.shade_smooth();
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::X)
    );

    mesh.selection_mut().set_level(ComponentTypes::FACE);
    mesh.selection_mut().select(&[face, FaceKey::null()]);
    mesh.shade_flat();
    for (key, corner) in &mesh.topology.loops {
        let expected = if corner.face == face {
            Vec3::Z
        } else {
            Vec3::X
        };
        assert_eq!(mesh.attributes.normals[key], expected);
    }

    mesh.selection_mut().set_level(ComponentTypes::VERTEX);
    mesh.selection_mut().set(&[vertex, VertKey::null()]);
    let previous_normals = mesh.attributes.normals.clone();
    mesh.shade_smooth();
    for (key, corner) in &mesh.topology.loops {
        let expected = if corner.vert == vertex {
            (Vec3::Y + Vec3::Z).normalize()
        } else {
            previous_normals[key]
        };
        assert!(mesh.attributes.normals[key].abs_diff_eq(expected, 1.0e-6));
    }
    assert_eq!(mesh.selection().verts().collect::<Vec<_>>(), vec![vertex]);
}

#[test]
fn recompute_selected_normals_reuse_face_normal_capacity_after_warmup() {
    let mut mesh = Mesh::from_buffers(triangle_buffers()).unwrap();
    let vertices: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&vertices);
    mesh.shade_flat();
    assert_eq!(mesh.face_normals.capacity(), 0);
    mesh.shade_smooth();

    let initial_capacity = mesh.face_normals.capacity();
    assert!(initial_capacity > 0);
    assert_eq!(mesh.face_normals.len(), 1);

    for height in [1.0, 2.0, 3.0] {
        mesh.attributes.positions[vertices[2]] = Vec3::new(0.0, 1.0, height);
        let expected = Vec3::new(0.0, -height, 1.0).normalize();
        mesh.shade_flat();
        mesh.shade_smooth();
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal.abs_diff_eq(expected, 1.0e-6))
        );
        assert_eq!(mesh.face_normals.capacity(), initial_capacity);
        assert_eq!(mesh.face_normals.len(), 1);
    }

    mesh.selection_mut().clear();
    mesh.attributes.positions[vertices[2]] = Vec3::Y;
    let previous_normals = mesh.attributes.normals.clone();
    mesh.shade_flat();
    mesh.shade_smooth();
    assert!(
        mesh.attributes
            .normals
            .iter()
            .all(|(key, &normal)| normal == previous_normals[key])
    );
    assert_eq!(mesh.face_normals.capacity(), initial_capacity);
    assert!(mesh.face_normals.is_empty());
}

#[test]
fn recompute_normals_handle_extreme_scales_and_collapsed_faces() {
    for scale in [
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        1.0e-30,
        1.0,
        1.0e30,
        f32::MAX,
    ] {
        let mut options = triangle_buffers();
        options
            .positions
            .iter_mut()
            .for_each(|position| *position *= scale);
        options.indices.reverse();
        options.normals = Some(vec![Vec3::X; 3]);
        let mut mesh = Mesh::from_buffers(options).unwrap();
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        let vertices: Vec<_> = mesh.topology.verts.keys().collect();
        Mesh::recompute_flat_normals(&mesh.topology, &mut mesh.attributes, faces.iter().copied());
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == -Vec3::Z)
        );
        Mesh::recompute_smooth_normals(
            &mesh.topology,
            &mut mesh.attributes,
            &mut mesh.face_normals,
            vertices.iter().copied(),
        );
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == -Vec3::Z)
        );

        for position in mesh.attributes.positions.values_mut() {
            *position = Vec3::ZERO;
        }
        Mesh::recompute_smooth_normals(
            &mesh.topology,
            &mut mesh.attributes,
            &mut mesh.face_normals,
            vertices.iter().copied(),
        );
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == Vec3::ZERO)
        );
        for normal in mesh.attributes.normals.values_mut() {
            *normal = Vec3::X;
        }
        Mesh::recompute_flat_normals(&mesh.topology, &mut mesh.attributes, faces.iter().copied());
        assert!(
            mesh.attributes
                .normals
                .values()
                .all(|&normal| normal == Vec3::ZERO)
        );
    }
}

#[test]
fn recompute_smooth_normals_handle_degenerate_faces_wires_and_isolated_vertices() {
    let mut options = triangle_buffers();
    options
        .positions
        .extend([2.0 * Vec3::X, Vec3::Z, 2.0 * Vec3::Z, 3.0 * Vec3::Z]);
    options.indices.extend([0, 1, 3]);
    options.normals = Some(vec![Vec3::X; 7]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let vertices: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.topology.insert_edge([vertices[4], vertices[5]]);
    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        vertices[4..].iter().copied(),
    );
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::X)
    );

    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        vertices.iter().copied(),
    );
    assert_eq!(mesh.attributes.normals.len(), 6);
    for (key, corner) in &mesh.topology.loops {
        let expected = if corner.vert == vertices[3] {
            Vec3::ZERO
        } else {
            Vec3::Z
        };
        assert_eq!(mesh.attributes.normals[key], expected);
    }
}

#[test]
fn recompute_smooth_normals_return_zero_when_faces_cancel() {
    let mut options = triangle_buffers();
    options.indices.extend([0, 2, 1]);
    let mut mesh = Mesh::from_buffers(options).unwrap();
    let vertices: Vec<_> = mesh.topology.verts.keys().collect();
    Mesh::recompute_smooth_normals(
        &mesh.topology,
        &mut mesh.attributes,
        &mut mesh.face_normals,
        vertices.iter().copied(),
    );
    assert!(
        mesh.attributes
            .normals
            .values()
            .all(|&normal| normal == Vec3::ZERO)
    );
}

#[test]
fn recompute_smooth_normals_use_interior_angles_of_concave_polygons() {
    for reversed in [false, true] {
        let mut polygon = vec![0, 1, 2, 3, 4, 5];
        let mut triangle = vec![3, 2, 6];
        if reversed {
            polygon.reverse();
            triangle.reverse();
        }
        polygon.extend(triangle);
        let mut mesh = Mesh::from_buffers(MeshBuffers {
            positions: vec![
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(2.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(1.0, 2.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
            ],
            normals: None,
            uvs: None,
            indices: polygon,
            face_vertex_counts: Some(vec![6, 3]),
        })
        .unwrap();
        let vertex = mesh.topology.verts.keys().nth(3).unwrap();
        Mesh::recompute_smooth_normals(
            &mesh.topology,
            &mut mesh.attributes,
            &mut mesh.face_normals,
            [vertex],
        );
        let expected = Vec3::new(0.0, -1.0, 3.0).normalize() * if reversed { -1.0 } else { 1.0 };
        for key in mesh.vert(vertex).unwrap().loops() {
            assert!(mesh.attributes.normals[key].abs_diff_eq(expected, 1.0e-6));
        }
    }
}

#[test]
fn recompute_smooth_normals_are_unchanged_by_splitting_a_planar_face() {
    for split in [false, true] {
        let mut mesh = Mesh::from_buffers(MeshBuffers {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y, Vec3::Z],
            normals: None,
            uvs: None,
            indices: if split {
                vec![0, 1, 2, 0, 2, 3, 1, 0, 4]
            } else {
                vec![0, 1, 2, 3, 1, 0, 4]
            },
            face_vertex_counts: Some(if split { vec![3, 3, 3] } else { vec![4, 3] }),
        })
        .unwrap();
        let vertex = mesh.topology.verts.keys().next().unwrap();
        Mesh::recompute_smooth_normals(
            &mesh.topology,
            &mut mesh.attributes,
            &mut mesh.face_normals,
            [vertex],
        );
        for key in mesh.vert(vertex).unwrap().loops() {
            assert!(
                mesh.attributes.normals[key].abs_diff_eq((Vec3::Y + Vec3::Z).normalize(), 1.0e-6)
            );
        }
    }
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
