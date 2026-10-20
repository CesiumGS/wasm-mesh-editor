use super::*;
use crate::{Mesh, MeshBuffers};
use event_emitter::Event;
use glam::Vec3;
use std::sync::mpsc::{Receiver, TryRecvError};

#[test]
fn selection_keys_store_typed_lists_and_support_dynamic_access() {
    use slotmap::{Key, SlotMap};

    let mut vertices = SlotMap::<VertKey, ()>::with_key();
    let first_vertex = vertices.insert(());
    let second_vertex = vertices.insert(());
    let edge = EdgeKey::null();
    let face = FaceKey::null();
    let mut keys = SelectionKeys::default();

    assert!(keys.is_empty());

    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        assert_eq!(keys.len(kind), 0);
        assert_eq!(keys.get(kind, 0), None);
        assert_eq!(keys.iter(kind).count(), 0);
    }

    keys.push(face.into());
    keys.push(first_vertex.into());
    keys.push(edge.into());
    keys.push(second_vertex.into());
    keys.push(first_vertex.into());

    assert!(!keys.is_empty());
    assert_eq!(keys.verts, vec![first_vertex, second_vertex, first_vertex]);
    assert_eq!(keys.edges, vec![edge]);
    assert_eq!(keys.faces, vec![face]);

    for (kind, expected) in [
        (
            ComponentType::Vertex,
            vec![
                first_vertex.into(),
                second_vertex.into(),
                first_vertex.into(),
            ],
        ),
        (ComponentType::Edge, vec![edge.into()]),
        (ComponentType::Face, vec![face.into()]),
    ] {
        assert_eq!(keys.len(kind), expected.len());
        assert_eq!(keys.iter(kind).collect::<Vec<_>>(), expected);

        for (index, key) in expected.iter().enumerate() {
            assert_eq!(keys.get(kind, index), Some(*key));
        }

        assert_eq!(keys.get(kind, expected.len()), None);
    }

    let snapshot = keys.clone();
    keys.verts.clear();
    assert_eq!(
        snapshot.verts,
        vec![first_vertex, second_vertex, first_vertex]
    );
}

fn triangle() -> Mesh {
    Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2],
        face_vertex_counts: None,
    })
    .unwrap()
}

fn assert_selection_event(
    changes: &Receiver<Event<MeshChange>>,
    added: &[ComponentKey],
    removed: &[ComponentKey],
) {
    let event = changes.try_recv().unwrap();
    let MeshChange::Selection(delta) = event.as_ref() else {
        panic!("expected a selection change");
    };
    for kind in [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ] {
        for (actual, expected) in [(&delta.added, added), (&delta.removed, removed)] {
            let expected: HashSet<_> = expected
                .iter()
                .copied()
                .filter(|key| key.kind() == kind)
                .collect();
            assert_eq!(actual.len(kind), expected.len());
            assert_eq!(actual.iter(kind).collect::<HashSet<_>>(), expected);
        }
    }
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn selection_events_include_propagation_once_per_action() {
    let mut mesh = triangle();
    let face = mesh.topology.faces().next().unwrap();
    let all: Vec<_> = mesh
        .topology
        .verts()
        .map(ComponentKey::Vert)
        .chain(mesh.topology.edges().map(ComponentKey::Edge))
        .chain(mesh.topology.faces().map(ComponentKey::Face))
        .collect();
    mesh.selection_mut().set_level(ComponentMask::FACE);
    let changes = mesh.subscribe();

    mesh.selection_mut().select(&[face]);
    assert_selection_event(&changes, &all, &[]);
    mesh.selection_mut().select(&[face]);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
    mesh.selection_mut().toggle(&[face]);
    assert_selection_event(&changes, &[], &all);
    mesh.selection_mut().toggle(&[face]);
    assert_selection_event(&changes, &all, &[]);
    mesh.selection_mut().deselect(&[face]);
    assert_selection_event(&changes, &[], &all);
    mesh.selection_mut().deselect(&[face]);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn set_and_clear_emit_only_net_changes() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts().collect();
    mesh.selection_mut().select(&verts[..1]);
    let changes = mesh.subscribe();

    mesh.selection_mut().set(&verts[..1]);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
    mesh.selection_mut().set(&verts[1..2]);
    assert_selection_event(&changes, &[verts[1].into()], &[verts[0].into()]);
    mesh.selection_mut().clear();
    assert_selection_event(&changes, &[], &[verts[1].into()]);
    mesh.selection_mut().clear();
    mesh.selection_mut().set::<VertKey>(&[]);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn normalization_preserves_net_changes_after_repeated_transitions() {
    let mesh = triangle();
    let keys: [ComponentKey; 3] = [
        mesh.topology.verts().next().unwrap().into(),
        mesh.topology.edges().next().unwrap().into(),
        mesh.topology.faces().next().unwrap().into(),
    ];

    for key in keys {
        for (added_count, removed_count) in [(0, 0), (1, 0), (0, 1), (1, 1), (2, 1), (1, 2)] {
            let mut delta = SelectionChange::default();

            for _ in 0..added_count {
                delta.added.push(key);
            }

            for _ in 0..removed_count {
                delta.removed.push(key);
            }

            for _ in 0..2 {
                delta.normalize();

                assert_eq!(
                    delta.added.iter(key.kind()).collect::<Vec<_>>(),
                    vec![key; usize::from(added_count > removed_count)]
                );
                assert_eq!(
                    delta.removed.iter(key.kind()).collect::<Vec<_>>(),
                    vec![key; usize::from(removed_count > added_count)]
                );
                assert_eq!(delta.is_empty(), added_count == removed_count);
            }
        }
    }
}

#[test]
fn normalization_keeps_component_kinds_independent() {
    use slotmap::Key;

    let vertex = VertKey::null();
    let edge = EdgeKey::null();
    let face = FaceKey::null();
    let mut delta = SelectionChange {
        added: SelectionKeys {
            verts: vec![vertex, vertex],
            edges: vec![edge],
            faces: vec![face],
        },
        removed: SelectionKeys {
            verts: vec![vertex],
            edges: vec![edge, edge],
            faces: vec![face],
        },
    };

    delta.normalize();

    assert_eq!(delta.added.verts, vec![vertex]);
    assert!(delta.removed.verts.is_empty());
    assert!(delta.added.edges.is_empty());
    assert_eq!(delta.removed.edges, vec![edge]);
    assert!(delta.added.faces.is_empty());
    assert!(delta.removed.faces.is_empty());
}

#[test]
fn set_emits_only_changes_to_overlapping_selections() {
    let mut mesh = triangle();
    let edges: Vec<_> = mesh.topology.edges().collect();
    mesh.selection_mut().set_level(ComponentMask::EDGE);
    mesh.selection_mut().select(&edges[..1]);
    let before: HashSet<_> = mesh.selection().selected().collect();
    let changes = mesh.subscribe();

    mesh.selection_mut().set(&edges[1..2]);

    let after: HashSet<_> = mesh.selection().selected().collect();
    let added: Vec<_> = after.difference(&before).copied().collect();
    let removed: Vec<_> = before.difference(&after).copied().collect();
    assert_selection_event(&changes, &added, &removed);
}

#[test]
fn rebuilds_preserving_selection_do_not_emit() {
    let mut mesh = triangle();
    let face = mesh.topology.faces().next().unwrap();
    mesh.selection_mut().set_level(ComponentMask::FACE);
    mesh.selection_mut().select(&[face]);
    let before: HashSet<_> = mesh.selection().selected().collect();
    let changes = mesh.subscribe();

    mesh.selection_mut().set(&[face]);
    mesh.selection_mut().set_level(ComponentMask::EDGE);
    mesh.selection_mut().set_level(ComponentMask::VERTEX);

    assert_eq!(mesh.selection().selected().collect::<HashSet<_>>(), before);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn level_changes_only_emit_selection_deltas() {
    let mut mesh = triangle();
    let vertex = mesh.topology.verts().next().unwrap();
    mesh.selection_mut().select(&[vertex]);
    let changes = mesh.subscribe();

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
    mesh.selection_mut().set_level(ComponentMask::FACE);
    assert_selection_event(&changes, &[], &[vertex.into()]);
    mesh.selection_mut().set_level(ComponentMask::EDGE);
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

fn quad_grid(columns: usize, rows: usize) -> Mesh {
    let positions = (0..=rows)
        .flat_map(|row| (0..=columns).map(move |column| Vec3::new(column as f32, row as f32, 0.0)))
        .collect();
    let stride = (columns + 1) as u32;
    let mut indices = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let vertex = row as u32 * stride + column as u32;
            indices.extend_from_slice(&[vertex, vertex + 1, vertex + stride + 1, vertex + stride]);
        }
    }
    Mesh::from_buffers(MeshBuffers {
        positions,
        normals: None,
        uvs: None,
        indices,
        face_vertex_counts: Some(vec![4; columns * rows]),
    })
    .unwrap()
}

#[test]
fn grow_shrink_and_clear_emit_complete_selection_changes() {
    for level in [
        ComponentMask::VERTEX,
        ComponentMask::EDGE,
        ComponentMask::FACE,
    ] {
        for step in [SelectionStep::Edge, SelectionStep::Face] {
            let mut mesh = quad_grid(5, 5);
            let face = mesh.topology.faces().nth(12).unwrap();
            let seed = if level == ComponentMask::FACE {
                ComponentKey::Face(face)
            } else if level == ComponentMask::EDGE {
                ComponentKey::Edge(mesh.topology.face_edges(face).next().unwrap())
            } else {
                ComponentKey::Vert(mesh.topology.face_verts(face).next().unwrap())
            };
            mesh.selection_mut().set_level(level);
            mesh.selection_mut().select(&[seed]);
            let before: HashSet<_> = mesh.selection().selected().collect();
            let changes = mesh.subscribe();

            mesh.selection_mut().grow(step);
            let grown: HashSet<_> = mesh.selection().selected().collect();
            assert!(grown.len() > before.len());
            let added: Vec<_> = grown.difference(&before).copied().collect();
            assert_selection_event(&changes, &added, &[]);

            mesh.selection_mut().shrink(step);
            let shrunk: HashSet<_> = mesh.selection().selected().collect();
            assert!(shrunk.len() < grown.len());
            let removed: Vec<_> = grown.difference(&shrunk).copied().collect();
            assert_selection_event(&changes, &[], &removed);

            assert!(!shrunk.is_empty());
            mesh.selection_mut().clear();
            assert_selection_event(&changes, &[], &shrunk.into_iter().collect::<Vec<_>>());
            mesh.selection_mut().clear();
            assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
        }
    }
}

#[test]
fn edge_step_shrink_retains_opposite_quad_corner() {
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        let mut mesh = quad_grid(1, 1);
        let verts: Vec<_> = mesh.topology.verts.keys().collect();
        mesh.selection_mut().select(&verts[..3]);
        mesh.selection_mut().shrink(step);
        let expected = if step == SelectionStep::Edge {
            vec![verts[0]]
        } else {
            vec![]
        };
        assert_eq!(mesh.selection().verts().collect::<Vec<_>>(), expected);
    }
}

#[test]
fn boundary_cache_is_lazy_and_shared_across_accessor_calls() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..1]);
    {
        let boundary = mesh.selection().boundary();
        assert_boundary_cache_initialized(&mesh, false);
        assert_eq!(boundary.inner_vertices(SelectionStep::Edge).count(), 1);
    }
    let cache = &mesh.selection.boundary;
    assert_eq!(
        cache.edge_step_vertices.inner.get(),
        Some(&HashSet::from([verts[0]]))
    );
    assert_eq!(
        boundary_cache_state(&mesh),
        [
            true, false, false, false, false, false, false, false, false, false
        ]
    );

    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.inner_vertices(SelectionStep::Face).count(), 1);
    assert_eq!(cache.mixed_faces.get().unwrap().len(), 1);
    assert!(cache.face_step_vertices.outer.get().is_none());
    assert!(std::ptr::eq(
        boundary.mixed_faces(),
        mesh.selection().boundary().mixed_faces()
    ));
    assert_eq!(boundary.outer_vertices(SelectionStep::Face).count(), 2);
    assert_eq!(boundary.outer_faces(SelectionStep::Face).count(), 0);
    assert!(cache.face_step_faces.outer.get().unwrap().is_empty());
    assert!(cache.face_step_faces.inner.get().is_none());
}

#[test]
fn boundary_queries_only_initialize_their_dependencies() {
    for query in 0..10 {
        let mut mesh = quad_grid(3, 3);
        let face = mesh.topology.faces.keys().nth(4).unwrap();
        mesh.selection_mut().set_level(ComponentMask::FACE);
        mesh.selection_mut().select(&[face]);
        let boundary = mesh.selection().boundary();
        match query {
            0 => boundary.inner_vertices(SelectionStep::Edge).count(),
            1 => boundary.outer_vertices(SelectionStep::Edge).count(),
            2 => boundary.inner_vertices(SelectionStep::Face).count(),
            3 => boundary.outer_vertices(SelectionStep::Face).count(),
            4 => boundary.inner_faces(SelectionStep::Edge).count(),
            5 => boundary.outer_faces(SelectionStep::Edge).count(),
            6 => boundary.inner_faces(SelectionStep::Face).count(),
            7 => boundary.outer_faces(SelectionStep::Face).count(),
            8 => boundary.mixed_faces().len(),
            9 => boundary.mixed_face_loops().len(),
            _ => unreachable!(),
        };
        let mut expected = [false; 10];
        expected[query] = true;
        expected[8] = matches!(query, 2 | 3 | 8 | 9);
        assert_eq!(boundary_cache_state(&mesh), expected, "query {query}");
    }
}

#[test]
fn mixed_face_loop_snapshots_are_shared_and_survive_invalidation() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let expected: HashSet<_> = mesh.topology.loops.keys().collect();
    mesh.selection_mut().select(&verts[..1]);

    let loops = Rc::clone(mesh.selection().boundary().mixed_face_loops());
    assert_eq!(loops.len(), expected.len());
    assert_eq!(loops.iter().copied().collect::<HashSet<_>>(), expected);
    assert!(loops.iter().any(|&key| {
        !mesh
            .selection
            .verts
            .contains(&mesh.topology.loops[key].vert)
    }));
    assert!(Rc::ptr_eq(
        &loops,
        mesh.selection().boundary().mixed_face_loops()
    ));

    mesh.selection_mut().select(&verts[1..]);
    assert!(mesh.selection.boundary.mixed_face_loops.get().is_none());

    let boundary = mesh.selection().boundary();
    let updated = boundary.mixed_face_loops();
    assert!(updated.is_empty());
    assert!(!Rc::ptr_eq(&loops, updated));
    assert_eq!(loops.iter().copied().collect::<HashSet<_>>(), expected);
}

#[test]
fn topology_and_attributes_remain_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<crate::Topology>();
    assert_send_sync::<crate::Attributes>();
}

fn populate_boundary_cache(mesh: &Mesh) {
    let boundary = mesh.selection().boundary();
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        boundary.inner_vertices(step).count();
        boundary.outer_vertices(step).count();
        boundary.inner_faces(step).count();
        boundary.outer_faces(step).count();
    }
    boundary.mixed_face_loops();
    assert_boundary_cache_initialized(mesh, true);
}

fn boundary_cache_state(mesh: &Mesh) -> [bool; 10] {
    let cache = &mesh.selection.boundary;
    [
        cache.edge_step_vertices.inner.get().is_some(),
        cache.edge_step_vertices.outer.get().is_some(),
        cache.face_step_vertices.inner.get().is_some(),
        cache.face_step_vertices.outer.get().is_some(),
        cache.edge_step_faces.inner.get().is_some(),
        cache.edge_step_faces.outer.get().is_some(),
        cache.face_step_faces.inner.get().is_some(),
        cache.face_step_faces.outer.get().is_some(),
        cache.mixed_faces.get().is_some(),
        cache.mixed_face_loops.get().is_some(),
    ]
}

fn assert_boundary_cache_initialized(mesh: &Mesh, expected: bool) {
    assert_eq!(boundary_cache_state(mesh), [expected; 10]);
}

#[test]
fn boundary_cache_invalidates_on_selection_edits() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    populate_boundary_cache(&mesh);

    mesh.selection_mut().select(&verts[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        1
    );

    mesh.selection_mut().grow(SelectionStep::Edge);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        3
    );

    mesh.selection_mut().shrink(SelectionStep::Face);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        0
    );

    mesh.selection_mut().set(&verts[..2]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        2
    );

    mesh.selection_mut().deselect(&verts[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .collect::<Vec<_>>(),
        vec![verts[1]]
    );

    mesh.selection_mut().toggle(&[verts[1], verts[2]]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .collect::<Vec<_>>(),
        vec![verts[2]]
    );

    mesh.selection_mut().clear();
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        0
    );

    mesh.selection_mut().select(&verts[..1]);
    populate_boundary_cache(&mesh);
    mesh.selection_mut().set_level(ComponentMask::FACE);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        0
    );
}

#[test]
fn boundary_cache_invalidates_on_cascaded_vertex_changes() {
    let mut mesh = quad_grid(2, 1);
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    mesh.selection_mut().set_level(ComponentMask::FACE);
    populate_boundary_cache(&mesh);

    mesh.selection_mut().select(&faces[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        2
    );

    mesh.selection_mut().deselect(&faces[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .count(),
        0
    );
}

#[test]
fn boundary_cache_survives_no_ops_and_position_edits() {
    use slotmap::Key;

    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().select(&verts[..1]);
    populate_boundary_cache(&mesh);

    mesh.selection_mut().select(&[verts[0], verts[0]]);
    mesh.selection_mut().deselect(&verts[1..]);
    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    mesh.selection_mut().toggle(&[face]);
    mesh.selection_mut().select(&[VertKey::null()]);
    assert_boundary_cache_initialized(&mesh, true);

    mesh.attributes.positions[verts[0]] += Vec3::Z;
    assert_boundary_cache_initialized(&mesh, true);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Face)
            .collect::<Vec<_>>(),
        vec![verts[0]]
    );
    assert_boundary_cache_initialized(&mesh, true);

    mesh.selection_mut().select(&verts);
    populate_boundary_cache(&mesh);
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        mesh.selection_mut().grow(step);
        mesh.selection_mut().shrink(step);
        assert_boundary_cache_initialized(&mesh, true);
        assert_eq!(mesh.selection().boundary().inner_vertices(step).count(), 0);
    }

    mesh.selection_mut().clear();
    populate_boundary_cache(&mesh);
    mesh.selection_mut().clear();
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        mesh.selection_mut().grow(step);
        mesh.selection_mut().shrink(step);
        assert_boundary_cache_initialized(&mesh, true);
    }
}

#[test]
fn boundary_cache_can_be_invalidated_after_topology_edits() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..1]);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection()
            .boundary()
            .outer_vertices(SelectionStep::Edge)
            .count(),
        2
    );

    mesh.topology.insert_edge([verts[0], verts[3]]);
    mesh.selection.invalidate_boundary();
    assert_boundary_cache_initialized(&mesh, false);
    assert_eq!(
        mesh.selection()
            .boundary()
            .outer_vertices(SelectionStep::Edge)
            .count(),
        3
    );
}

#[test]
fn grow_adds_one_edge_ring_at_a_time() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().grow(SelectionStep::Edge);
    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert!(mesh.selection().is_empty());

    mesh.selection_mut().select(&[verts[12]]);
    mesh.selection_mut().grow(SelectionStep::Edge);
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        [7, 11, 12, 13, 17].map(|index| verts[index]).into()
    );
    assert_eq!(mesh.selection().edges().count(), 4);
    assert_eq!(mesh.selection().faces().count(), 0);

    for count in [13, 21, 25] {
        mesh.selection_mut().grow(SelectionStep::Edge);
        assert_eq!(mesh.selection().verts().count(), count);
    }
    assert_eq!(mesh.selection().faces().count(), 16);
    mesh.selection_mut().grow(SelectionStep::Edge);
    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert_eq!(mesh.selection().verts().count(), 25);
    assert_eq!(mesh.selection().faces().count(), 16);
    assert_eq!(mesh.selection().level(), ComponentMask::VERTEX);
}

#[test]
fn shrink_removes_the_boundary_and_recomputes_it() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let selected = [6, 7, 8, 11, 12, 13, 16, 17, 18].map(|index| verts[index]);
    mesh.selection_mut().select(&selected);
    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert_eq!(
        mesh.selection().verts().collect::<Vec<_>>(),
        vec![verts[12]]
    );
    assert_eq!(mesh.selection().edges().count(), 0);
    assert_eq!(mesh.selection().faces().count(), 0);
    assert_eq!(
        mesh.selection()
            .boundary()
            .inner_vertices(SelectionStep::Edge)
            .count(),
        1
    );
    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert!(mesh.selection().is_empty());
    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert!(mesh.selection().is_empty());
}

#[test]
fn boundary_of_empty_partial_and_full_selection() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let face = mesh.topology.faces.keys().next().unwrap();

    for step in [SelectionStep::Edge, SelectionStep::Face] {
        for selected in [&verts[..0], &verts[..]] {
            mesh.selection_mut().set(selected);
            let boundary = mesh.selection().boundary();
            for _ in 0..2 {
                assert_eq!(boundary.inner_vertices(step).count(), 0);
                assert_eq!(boundary.outer_vertices(step).count(), 0);
                assert_eq!(boundary.inner_faces(step).count(), 0);
                assert_eq!(boundary.outer_faces(step).count(), 0);
                assert!(boundary.mixed_faces().is_empty());
            }
        }

        mesh.selection_mut().set(&verts[..1]);
        let boundary = mesh.selection().boundary();
        assert_eq!(
            boundary.inner_vertices(step).collect::<Vec<_>>(),
            vec![verts[0]]
        );
        assert_eq!(boundary.inner_faces(step).count(), 0);
        assert_eq!(
            boundary.outer_vertices(step).collect::<HashSet<_>>(),
            HashSet::from([verts[1], verts[2]])
        );
        assert_eq!(boundary.outer_faces(step).count(), 0);
        assert_eq!(boundary.mixed_faces(), &HashSet::from([face]));
        assert_eq!(mesh.selection().len(), 1);
    }
}

#[test]
fn boundary_rings_partition_a_selected_grid_patch() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let selected = [6, 7, 8, 11, 12, 13, 16, 17, 18].map(|index| verts[index]);
    mesh.selection_mut().select(&selected);
    let boundary = mesh.selection().boundary();
    let inner_faces: HashSet<_> = [5, 6, 9, 10].map(|index| faces[index]).into();
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        let outer_faces: HashSet<_> = faces
            .iter()
            .enumerate()
            .filter(|(index, key)| {
                !inner_faces.contains(key)
                    && (step == SelectionStep::Face || ![0, 3, 12, 15].contains(index))
            })
            .map(|(_, &key)| key)
            .collect();
        let outer_vertices: HashSet<_> = verts
            .iter()
            .enumerate()
            .filter(|(index, key)| {
                !selected.contains(key)
                    && (step == SelectionStep::Face || ![0, 4, 20, 24].contains(index))
            })
            .map(|(_, &key)| key)
            .collect();
        assert_eq!(
            boundary.outer_faces(step).collect::<HashSet<_>>(),
            outer_faces
        );
        assert_eq!(
            boundary.inner_faces(step).collect::<HashSet<_>>(),
            inner_faces
        );
        assert_eq!(
            boundary.inner_vertices(step).collect::<HashSet<_>>(),
            [6, 7, 8, 11, 13, 16, 17, 18]
                .map(|index| verts[index])
                .into()
        );
        assert_eq!(
            boundary.outer_vertices(step).collect::<HashSet<_>>(),
            outer_vertices
        );
    }
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        selected.into()
    );
}

#[test]
fn vertex_steps_differ_on_polygon_corners() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().select(&verts[..1]);
    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.outer_faces(SelectionStep::Face).count(), 0);
    assert_eq!(boundary.mixed_faces(), &HashSet::from([face]));
    assert_eq!(
        boundary
            .outer_vertices(SelectionStep::Edge)
            .collect::<HashSet<_>>(),
        HashSet::from([verts[1], verts[2]])
    );
    assert_eq!(
        boundary
            .outer_vertices(SelectionStep::Face)
            .collect::<HashSet<_>>(),
        HashSet::from([verts[1], verts[2], verts[3]])
    );

    mesh.selection_mut().grow(SelectionStep::Face);
    assert_eq!(mesh.selection().verts().count(), 4);
    assert!(mesh.selection().contains(face));
}

#[test]
fn selected_faces_need_no_interior_vertex_to_have_a_boundary() {
    let mut mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, -Vec3::Y, -Vec3::X],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 1, 0, 3, 2, 1, 4, 0, 2, 5],
        face_vertex_counts: None,
    })
    .unwrap();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    mesh.selection_mut().select(&verts[..3]);
    let boundary = mesh.selection().boundary();
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        assert_eq!(
            boundary.inner_faces(step).collect::<Vec<_>>(),
            vec![faces[0]]
        );
        assert_eq!(
            boundary.outer_faces(step).collect::<HashSet<_>>(),
            faces[1..].iter().copied().collect()
        );
        assert_eq!(boundary.inner_vertices(step).count(), 3);
    }
    assert!(mesh.selection().contains(faces[0]));
}

#[test]
fn boundary_handles_non_manifold_faces_wires_and_isolated_vertices() {
    let mut mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            -Vec3::Y,
            -Vec3::X,
            -Vec3::Z,
        ],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 1, 0, 3, 0, 1, 4],
        face_vertex_counts: None,
    })
    .unwrap();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    mesh.topology.insert_edge([verts[0], verts[5]]);
    let vertex = mesh.vert(verts[0]).unwrap();
    assert_eq!(vertex.faces().count(), 3);
    assert_eq!(
        vertex.faces().collect::<HashSet<_>>(),
        faces.iter().copied().collect()
    );
    assert_eq!(vertex.neighbors().count(), 5);
    assert_eq!(
        vertex.neighbors().collect::<HashSet<_>>(),
        verts[1..6].iter().copied().collect()
    );
    let wire_vertex = mesh.vert(verts[5]).unwrap();
    assert_eq!(wire_vertex.faces().count(), 0);
    assert_eq!(wire_vertex.neighbors().collect::<Vec<_>>(), vec![verts[0]]);
    let isolated = mesh.vert(verts[6]).unwrap();
    assert_eq!(isolated.faces().count(), 0);
    assert_eq!(isolated.neighbors().count(), 0);

    mesh.selection_mut()
        .select(&[verts[0], verts[1], verts[2], verts[6]]);
    let boundary = mesh.selection().boundary();
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        assert_eq!(
            boundary.inner_vertices(step).collect::<HashSet<_>>(),
            HashSet::from([verts[0], verts[1]])
        );
        assert_eq!(
            boundary.outer_vertices(step).collect::<HashSet<_>>(),
            HashSet::from([verts[3], verts[4], verts[5]])
        );
        assert_eq!(
            boundary.inner_faces(step).collect::<Vec<_>>(),
            vec![faces[0]]
        );
        assert_eq!(
            boundary.outer_faces(step).collect::<HashSet<_>>(),
            HashSet::from([faces[1], faces[2]])
        );
    }

    mesh.selection_mut().shrink(SelectionStep::Edge);
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        HashSet::from([verts[2], verts[6]])
    );
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        mesh.selection_mut().set(&verts[..5]);
        assert_eq!(
            mesh.selection()
                .boundary()
                .inner_vertices(step)
                .collect::<Vec<_>>(),
            vec![verts[0]]
        );
        assert!(mesh.selection().boundary().mixed_faces().is_empty());
        mesh.selection_mut().grow(step);
        mesh.selection_mut().shrink(step);
        assert_eq!(mesh.selection().verts().count(), 6);
        assert!(mesh.selection().contains(verts[5]));

        mesh.selection_mut().set(&verts[5..]);
        let boundary = mesh.selection().boundary();
        assert_eq!(
            boundary.inner_vertices(step).collect::<Vec<_>>(),
            vec![verts[5]]
        );
        assert_eq!(
            boundary.outer_vertices(step).collect::<Vec<_>>(),
            vec![verts[0]]
        );
        assert_eq!(boundary.inner_faces(step).count(), 0);
        assert_eq!(boundary.outer_faces(step).count(), 0);
        assert!(boundary.mixed_faces().is_empty());
        mesh.selection_mut().grow(step);
        assert_eq!(
            mesh.selection().verts().collect::<HashSet<_>>(),
            HashSet::from([verts[0], verts[5], verts[6]])
        );
        mesh.selection_mut().shrink(step);
        assert_eq!(
            mesh.selection().verts().collect::<HashSet<_>>(),
            HashSet::from([verts[5], verts[6]])
        );
        mesh.selection_mut().shrink(step);
        assert_eq!(mesh.selection().verts().collect::<Vec<_>>(), vec![verts[6]]);
    }
}

#[test]
fn grow_preserves_each_selection_mode_and_propagates_components() {
    for bits in 1..=ComponentMask::all().bits() {
        let level = ComponentMask::from_bits_retain(bits);
        let mut mesh = quad_grid(3, 3);
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        mesh.selection_mut().set_level(ComponentMask::FACE);
        mesh.selection_mut().select(&[faces[4]]);
        mesh.selection_mut().set_level(level);
        mesh.selection_mut().grow(SelectionStep::Edge);
        assert_eq!(mesh.selection().level(), level);
        assert_eq!(mesh.selection().verts().count(), 12, "{level:?}");
        assert_eq!(mesh.selection().edges().count(), 16, "{level:?}");
        assert_eq!(
            mesh.selection().faces().collect::<HashSet<_>>(),
            [1, 3, 4, 5, 7].map(|index| faces[index]).into(),
            "{level:?}"
        );
    }
}

#[test]
fn shrink_uses_faces_only_in_face_only_mode() {
    for bits in 0..=ComponentMask::all().bits() {
        let level = ComponentMask::from_bits_retain(bits);
        let mut mesh = quad_grid(4, 4);
        let verts: Vec<_> = mesh.topology.verts.keys().collect();
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        mesh.selection_mut().set_level(ComponentMask::FACE);
        mesh.selection_mut()
            .select(&[faces[5], faces[6], faces[9], faces[10]]);
        mesh.selection_mut().set_level(level);
        mesh.selection_mut().shrink(SelectionStep::Edge);
        assert_eq!(mesh.selection().level(), level);
        assert_eq!(mesh.selection().edges().count(), 0, "{level:?}");
        assert_eq!(mesh.selection().faces().count(), 0, "{level:?}");
        if level.intersects(ComponentMask::VERTEX | ComponentMask::EDGE) {
            assert_eq!(
                mesh.selection().verts().collect::<Vec<_>>(),
                vec![verts[12]]
            );
        } else {
            assert!(mesh.selection().is_empty(), "{level:?}");
        }
    }
}

#[test]
fn boundary_operations_preserve_unrelated_partial_edge_selections() {
    for level in [
        ComponentMask::EDGE,
        ComponentMask::EDGE | ComponentMask::FACE,
        ComponentMask::VERTEX | ComponentMask::EDGE,
        ComponentMask::all(),
    ] {
        for grow in [true, false] {
            let mut mesh = Mesh::from_buffers(MeshBuffers {
                positions: vec![
                    Vec3::ZERO,
                    Vec3::X,
                    Vec3::Y,
                    Vec3::Z,
                    Vec3::Z + Vec3::X,
                    Vec3::Z + Vec3::Y,
                ],
                normals: None,
                uvs: None,
                indices: vec![0, 1, 2, 3, 4, 5],
                face_vertex_counts: None,
            })
            .unwrap();
            let faces: Vec<_> = mesh.topology.faces.keys().collect();
            let first_edges: Vec<_> = mesh.face(faces[0]).unwrap().edges().collect();
            let second_edge = mesh.face(faces[1]).unwrap().edges().next().unwrap();
            mesh.selection_mut().set_level(level);
            mesh.selection_mut()
                .select(&[first_edges[0], first_edges[1], second_edge]);
            assert!(!mesh.selection().contains(first_edges[2]));
            assert!(!mesh.selection().contains(faces[0]));

            if grow {
                mesh.selection_mut().grow(SelectionStep::Edge);
            } else {
                mesh.selection_mut().shrink(SelectionStep::Edge);
            }

            let selection = mesh.selection();
            assert_eq!(selection.level(), level);
            assert!(selection.contains(first_edges[0]));
            assert!(selection.contains(first_edges[1]));
            assert!(
                !selection.contains(first_edges[2]),
                "{level:?}, grow={grow}"
            );
            assert!(!selection.contains(faces[0]), "{level:?}, grow={grow}");
            assert_eq!(selection.verts().count(), if grow { 6 } else { 3 });
            assert_eq!(selection.edges().count(), if grow { 5 } else { 2 });
            assert_eq!(selection.contains(faces[1]), grow);
        }
    }
}

#[test]
fn boundary_operations_without_a_boundary_do_not_rebuild_selection() {
    let mut mesh = triangle();
    let edges: Vec<_> = mesh.topology.edges.keys().collect();
    mesh.selection_mut().set_level(ComponentMask::EDGE);
    mesh.selection_mut().select(&edges[..2]);
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        mesh.selection_mut().grow(step);
        mesh.selection_mut().shrink(step);
    }
    assert_eq!(mesh.selection().level(), ComponentMask::EDGE);
    assert_eq!(mesh.selection().verts().count(), 3);
    assert_eq!(mesh.selection().edges().count(), 2);
    assert_eq!(mesh.selection().faces().count(), 0);
}

#[test]
fn face_step_grow_adds_one_vertex_ring_at_a_time() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&[verts[12]]);
    mesh.selection_mut().grow(SelectionStep::Face);
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        [6, 7, 8, 11, 12, 13, 16, 17, 18]
            .map(|index| verts[index])
            .into()
    );
    mesh.selection_mut().grow(SelectionStep::Face);
    assert_eq!(mesh.selection().verts().count(), 25);
    assert_eq!(mesh.selection().faces().count(), 16);
}

#[test]
fn face_step_includes_diagonally_adjacent_faces() {
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        let mut mesh = quad_grid(3, 3);
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        mesh.selection_mut().set_level(ComponentMask::FACE);
        mesh.selection_mut().select(&[faces[4]]);
        mesh.selection_mut().grow(step);
        let expected: HashSet<_> = if step == SelectionStep::Edge {
            [1, 3, 4, 5, 7].map(|index| faces[index]).into()
        } else {
            faces.iter().copied().collect()
        };
        assert_eq!(mesh.selection().faces().collect::<HashSet<_>>(), expected);

        mesh.selection_mut().set(&faces[1..]);
        mesh.selection_mut().shrink(step);
        let expected: HashSet<_> = if step == SelectionStep::Edge {
            [2, 4, 5, 6, 7, 8].map(|index| faces[index]).into()
        } else {
            [2, 5, 6, 7, 8].map(|index| faces[index]).into()
        };
        assert_eq!(mesh.selection().faces().collect::<HashSet<_>>(), expected);
        assert_eq!(mesh.selection().level(), ComponentMask::FACE);
    }
}

#[test]
fn boundary_sides_swap_when_selection_is_inverted() {
    for level in [ComponentMask::VERTEX, ComponentMask::FACE] {
        let mut mesh = quad_grid(2, 2);
        mesh.selection_mut().set_level(level);
        let components: Vec<_> = if level == ComponentMask::VERTEX {
            mesh.topology.verts.keys().map(ComponentKey::Vert).collect()
        } else {
            mesh.topology.faces.keys().map(ComponentKey::Face).collect()
        };
        for step in [SelectionStep::Edge, SelectionStep::Face] {
            let sides = |mesh: &Mesh| -> (HashSet<ComponentKey>, HashSet<ComponentKey>) {
                let boundary = mesh.selection().boundary();
                if level == ComponentMask::VERTEX {
                    (
                        boundary
                            .inner_vertices(step)
                            .map(ComponentKey::Vert)
                            .collect(),
                        boundary
                            .outer_vertices(step)
                            .map(ComponentKey::Vert)
                            .collect(),
                    )
                } else {
                    (
                        boundary.inner_faces(step).map(ComponentKey::Face).collect(),
                        boundary.outer_faces(step).map(ComponentKey::Face).collect(),
                    )
                }
            };
            for mask in 0..(1usize << components.len()) {
                let selected: Vec<_> = components
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, &key)| key)
                    .collect();
                let unselected: Vec<_> = components
                    .iter()
                    .copied()
                    .filter(|key| !selected.contains(key))
                    .collect();
                mesh.selection_mut().set(&selected);
                let (inner, outer) = sides(&mesh);
                assert!(inner.is_disjoint(&outer));
                mesh.selection_mut().set(&unselected);
                assert_eq!(sides(&mesh), (outer, inner), "{level:?} {step:?} {mask}");
            }
        }
    }
}

#[test]
fn unselected_faces_with_fully_selected_vertices_are_not_mixed() {
    let mut mesh = quad_grid(3, 3);
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let surrounding: Vec<_> = faces
        .iter()
        .copied()
        .filter(|&key| key != faces[4])
        .collect();
    mesh.selection_mut().set_level(ComponentMask::FACE);
    mesh.selection_mut().select(&surrounding);
    assert_eq!(mesh.selection().verts().count(), 16);
    assert!(!mesh.selection().contains(faces[4]));

    let boundary = mesh.selection().boundary();
    assert!(boundary.mixed_faces().is_empty());
    for step in [SelectionStep::Edge, SelectionStep::Face] {
        assert_eq!(
            boundary.outer_faces(step).collect::<Vec<_>>(),
            vec![faces[4]]
        );
        assert_eq!(boundary.inner_vertices(step).count(), 0);
        assert_eq!(boundary.outer_vertices(step).count(), 0);
    }
}

#[test]
fn translation_normal_sets_include_every_corner_of_changed_faces() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let selected = [6, 7, 8, 11, 12, 13, 16, 17, 18].map(|index| verts[index]);
    mesh.selection_mut().select(&selected);
    populate_boundary_cache(&mesh);
    let boundary = mesh.selection().boundary();
    let mixed = boundary.mixed_faces().clone();
    let smooth: HashSet<_> = boundary
        .inner_vertices(SelectionStep::Face)
        .chain(boundary.outer_vertices(SelectionStep::Face))
        .collect();
    let mut mixed_vertices = HashSet::new();
    for &key in &mixed {
        mixed_vertices.extend(mesh.face(key).unwrap().verts());
    }
    assert_eq!(smooth, mixed_vertices);
    assert_eq!(
        smooth,
        verts
            .iter()
            .copied()
            .filter(|&key| key != verts[12])
            .collect()
    );
    assert_eq!(mixed.len(), 12);
    let previous_normals: Vec<_> = mesh
        .topology
        .faces
        .keys()
        .map(|key| (key, mesh.face(key).unwrap().normal()))
        .collect();

    for key in selected {
        mesh.attributes.positions[key] += Vec3::Z;
    }
    let changed: HashSet<_> = previous_normals
        .into_iter()
        .filter(|&(key, normal)| mesh.face(key).unwrap().normal() != normal)
        .map(|(key, _)| key)
        .collect();
    assert_eq!(changed, mixed);
    assert_boundary_cache_initialized(&mesh, true);
    assert_eq!(mesh.selection().boundary().mixed_faces(), &mixed);
}

#[test]
fn accessors_expose_each_selected_kind() {
    let mut mesh = triangle();
    assert_eq!(mesh.selection().level(), ComponentMask::VERTEX);
    assert!(mesh.selection().is_empty());
    let vert = mesh.topology.verts.keys().next().unwrap();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let face = mesh.topology.faces.keys().next().unwrap();
    std::rc::Rc::make_mut(&mut mesh.selection.verts).insert(vert);
    mesh.selection.edges.insert(edge);
    mesh.selection.faces.insert(face);

    let selection = mesh.selection_mut();
    let view = selection.view();
    assert_eq!(view.len(), 3);
    assert!(!view.is_empty());
    assert!(view.contains(vert));
    assert!(view.contains(edge));
    assert!(view.contains(face));
    assert_eq!(view.verts().collect::<Vec<_>>(), vec![vert]);
    assert_eq!(view.edges().collect::<Vec<_>>(), vec![edge]);
    assert_eq!(view.faces().collect::<Vec<_>>(), vec![face]);
    assert_eq!(
        view.selected().collect::<Vec<_>>(),
        vec![vert.into(), edge.into(), face.into()]
    );
}

#[test]
fn vertex_snapshots_survive_selection_edits() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..1]);
    let first = std::rc::Rc::clone(&mesh.selection.verts);

    mesh.selection_mut().select(&verts[1..2]);
    assert!(!std::rc::Rc::ptr_eq(&first, &mesh.selection.verts));
    assert_eq!(*first, HashSet::from([verts[0]]));
    let second = std::rc::Rc::clone(&mesh.selection.verts);

    mesh.selection_mut().deselect(&verts[..1]);
    assert_eq!(*second, HashSet::from([verts[0], verts[1]]));
    assert_eq!(*mesh.selection.verts, HashSet::from([verts[1]]));
    let third = std::rc::Rc::clone(&mesh.selection.verts);

    mesh.selection_mut().clear();
    assert!(mesh.selection().is_empty());
    assert_eq!(*third, HashSet::from([verts[1]]));

    mesh.selection_mut().set(&verts[2..]);
    let fourth = std::rc::Rc::clone(&mesh.selection.verts);
    mesh.selection_mut().set(&verts[..1]);
    assert_eq!(*fourth, HashSet::from([verts[2]]));
    assert_eq!(*mesh.selection.verts, HashSet::from([verts[0]]));
    assert_eq!(*first, HashSet::from([verts[0]]));
}

#[test]
fn vertex_selection_noops_preserve_shared_storage() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let empty = std::rc::Rc::clone(&mesh.selection.verts);

    mesh.selection_mut().clear();
    assert!(std::rc::Rc::ptr_eq(&empty, &mesh.selection.verts));

    mesh.selection_mut().select(&verts[..1]);
    let selected = std::rc::Rc::clone(&mesh.selection.verts);

    mesh.selection_mut().select(&verts[..1]);
    mesh.selection_mut().deselect(&verts[1..2]);
    assert!(std::rc::Rc::ptr_eq(&selected, &mesh.selection.verts));
}

#[test]
fn clearing_unique_vertex_selection_reuses_storage() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts);
    let storage = std::rc::Rc::as_ptr(&mesh.selection.verts);
    let capacity = mesh.selection.verts.capacity();

    mesh.selection_mut().clear();

    assert!(mesh.selection().is_empty());
    assert_eq!(std::rc::Rc::as_ptr(&mesh.selection.verts), storage);
    assert_eq!(mesh.selection.verts.capacity(), capacity);
}

#[test]
fn direct_vertices_bubble_through_edges_to_faces() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..2]);
    assert_eq!(mesh.selection().len(), 3);
    assert_eq!(mesh.selection().edges().count(), 1);
    mesh.selection_mut().select(&verts[2..]);
    assert_eq!(mesh.selection().len(), 7);
    assert_eq!(mesh.selection().faces().count(), 1);
    mesh.selection_mut().deselect(&verts[..1]);
    assert_eq!(mesh.selection().verts().count(), 2);
    assert_eq!(mesh.selection().edges().count(), 1);
    assert_eq!(mesh.selection().faces().count(), 0);
}

#[test]
fn cascaded_vertices_do_not_promote_untouched_edges() {
    let mut mesh = triangle();
    let edges: Vec<_> = mesh.topology.edges.keys().collect();
    mesh.selection_mut().set_level(ComponentMask::EDGE);
    mesh.selection_mut().select(&edges[..2]);
    assert_eq!(mesh.selection().verts().count(), 3);
    assert_eq!(mesh.selection().edges().count(), 2);
    assert!(!mesh.selection().contains(edges[2]));
    assert_eq!(mesh.selection().faces().count(), 0);
    mesh.selection_mut().select(&edges[2..]);
    assert_eq!(mesh.selection().faces().count(), 1);
    mesh.selection_mut().deselect(&edges[..1]);
    assert_eq!(mesh.selection().verts().count(), 3);
    assert_eq!(mesh.selection().edges().count(), 2);
    assert_eq!(mesh.selection().faces().count(), 0);
}

#[test]
fn face_selection_cascades_and_removal_clears_orphans() {
    let mut mesh = triangle();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().set_level(ComponentMask::FACE);
    mesh.selection_mut().select(&[face]);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().deselect(&[face]);
    assert!(mesh.selection().is_empty());
}

#[test]
fn mixed_modes_filter_direct_input_and_rebuild_survivors() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut()
        .set_level(ComponentMask::VERTEX | ComponentMask::EDGE);
    mesh.selection_mut()
        .select(&[ComponentKey::Face(face), edge.into()]);
    assert_eq!(mesh.selection().len(), 3);
    mesh.selection_mut().select(&verts);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set_level(ComponentMask::FACE);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().deselect(&verts);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set_level(ComponentMask::empty());
    assert!(mesh.selection().is_empty());
    mesh.selection_mut().select(&[face]);
    assert!(mesh.selection().is_empty());
}

#[test]
fn toggle_deduplicates_and_uses_pre_cascade_membership() {
    let mut mesh = triangle();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let verts = mesh.topology.edges[edge].verts;
    mesh.selection_mut().set_level(ComponentMask::all());
    mesh.selection_mut()
        .toggle(&[ComponentKey::Vert(verts[0]), edge.into(), verts[0].into()]);
    assert_eq!(mesh.selection().len(), 3);
    mesh.selection_mut().toggle(&[verts[0], verts[0]]);
    assert!(!mesh.selection().contains(verts[0]));
    assert!(!mesh.selection().contains(edge));
    assert!(mesh.selection().contains(verts[1]));
}

#[test]
fn set_replaces_selection_and_clear_ignores_mode() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts);
    mesh.selection_mut().set(&verts[..1]);
    assert_eq!(mesh.selection().len(), 1);
    mesh.selection_mut().clear();
    assert!(mesh.selection().is_empty());
}

#[test]
fn unchanged_modes_and_repeated_inputs_do_not_rebuild_selection() {
    let mut mesh = triangle();
    let edges: Vec<_> = mesh.topology.edges.keys().collect();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let mode = ComponentMask::VERTEX | ComponentMask::EDGE;
    mesh.selection_mut().set_level(mode);
    mesh.selection_mut().select(&edges[..2]);
    mesh.selection_mut().select(&verts);
    mesh.selection_mut().select(&edges[..2]);
    mesh.selection_mut().deselect(&edges[2..]);
    mesh.selection_mut().set_level(mode);
    assert_eq!(mesh.selection().len(), 5);
    assert!(!mesh.selection().contains(edges[2]));

    mesh.selection_mut().set_level(ComponentMask::VERTEX);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set(&verts[..1]);
    mesh.selection_mut().set_level(ComponentMask::FACE);
    assert!(mesh.selection().is_empty());
}

#[test]
fn mixed_toggle_processes_removals_before_additions() {
    let mut mesh = triangle();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let verts = mesh.topology.edges[edge].verts;
    mesh.selection_mut().set_level(ComponentMask::all());
    mesh.selection_mut().select(&verts[..1]);
    let changes = mesh.subscribe();

    mesh.selection_mut()
        .toggle(&[ComponentKey::Vert(verts[0]), edge.into()]);
    assert_selection_event(&changes, &[verts[1].into()], &[]);
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        HashSet::from(verts)
    );
    assert!(!mesh.selection().contains(edge));
    assert_eq!(mesh.selection().len(), 2);
}

#[test]
fn face_removal_preserves_shared_geometry_and_unrelated_selections() {
    let mut mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, -Vec3::Y, -Vec3::X],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 1, 0, 3, 0, 1, 4],
        face_vertex_counts: None,
    })
    .unwrap();
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().set_level(ComponentMask::all());
    mesh.selection_mut().select(&faces);
    mesh.selection_mut().select(&verts[5..]);
    mesh.selection_mut().deselect(&faces[..1]);
    assert_eq!(mesh.selection().verts().count(), 5);
    assert!(!mesh.selection().contains(verts[2]));
    assert_eq!(mesh.selection().edges().count(), 5);
    assert_eq!(mesh.selection().faces().count(), 2);
    mesh.selection_mut().deselect(&faces[1..]);
    assert_eq!(
        mesh.selection().selected().collect::<Vec<_>>(),
        vec![verts[5].into()]
    );

    mesh.selection_mut().select(&faces);
    mesh.selection_mut().deselect(&verts[..1]);
    assert_eq!(mesh.selection().faces().count(), 0);
    assert_eq!(mesh.selection().edges().count(), 3);
    assert_eq!(mesh.selection().verts().count(), 5);
}

#[test]
fn polygon_wire_and_isolated_vertices_follow_the_same_rules() {
    let mut mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y, Vec3::Z, -Vec3::X],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 3],
        face_vertex_counts: Some(vec![4]),
    })
    .unwrap();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let wire = mesh.topology.insert_edge([verts[3], verts[4]]);
    mesh.selection_mut().select(&verts[..3]);
    assert_eq!(mesh.selection().faces().count(), 0);
    mesh.selection_mut().select(&verts[3..]);
    assert_eq!(mesh.selection().len(), 12);
    assert!(mesh.selection().contains(wire));
    mesh.selection_mut().deselect(&verts[4..]);
    assert!(!mesh.selection().contains(wire));
    assert_eq!(mesh.selection().len(), 9);
    mesh.selection_mut().set_level(ComponentMask::FACE);
    mesh.selection_mut().clear();
    assert!(mesh.selection().is_empty());
}

#[test]
fn missing_keys_and_disabled_toggle_inputs_are_ignored() {
    use slotmap::Key;

    let mut mesh = triangle();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().toggle(&[face, face]);
    assert!(mesh.selection().is_empty());
    let stale = mesh.topology.verts.insert(crate::Vert { edge: None });
    mesh.topology.verts.remove(stale);
    let missing = [
        ComponentKey::Vert(VertKey::null()),
        ComponentKey::Edge(EdgeKey::null()),
        ComponentKey::Face(FaceKey::null()),
        ComponentKey::Vert(stale),
    ];
    mesh.selection_mut().set_level(ComponentMask::all());
    mesh.selection_mut().select(&missing);
    mesh.selection_mut().toggle(&missing);
    mesh.selection_mut().deselect(&missing);
    assert!(mesh.selection().is_empty());
}
