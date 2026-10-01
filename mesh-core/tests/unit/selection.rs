use super::*;
use crate::{Mesh, MeshBuffers};
use glam::Vec3;

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
fn boundary_cache_is_lazy_and_shared_across_accessor_calls() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..1]);
    {
        let boundary = mesh.selection().boundary();
        assert!(mesh.selection.boundary.vertices.get().is_none());
        assert_eq!(boundary.vertices().count(), 1);
    }
    let cache = &mesh.selection.boundary;
    assert_eq!(cache.vertices.get(), Some(&HashSet::from([verts[0]])));
    assert!(cache.inner_vertices.get().is_none());
    assert!(cache.outer_vertices.get().is_none());
    assert!(cache.inner_faces.get().is_none());
    assert!(cache.outer_faces.get().is_none());

    assert_eq!(mesh.selection().boundary().outer_faces().count(), 1);
    assert_eq!(cache.outer_vertices.get().unwrap().len(), 2);
    assert_eq!(cache.outer_faces.get().unwrap().len(), 1);
    assert!(cache.inner_vertices.get().is_none());
    assert!(cache.inner_faces.get().is_none());
    assert_eq!(mesh.selection().boundary().inner_faces().count(), 0);
    assert!(cache.inner_vertices.get().unwrap().is_empty());
    assert!(cache.inner_faces.get().unwrap().is_empty());
}

#[test]
fn boundary_cache_preserves_mesh_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Mesh>();
}

fn populate_boundary_cache(mesh: &Mesh) {
    let boundary = mesh.selection().boundary();
    boundary.vertices().count();
    boundary.inner_vertices().count();
    boundary.outer_vertices().count();
    boundary.inner_faces().count();
    boundary.outer_faces().count();
    assert_boundary_cache_initialized(mesh, true);
}

fn assert_boundary_cache_initialized(mesh: &Mesh, expected: bool) {
    let cache = &mesh.selection.boundary;
    assert_eq!(cache.vertices.get().is_some(), expected);
    assert_eq!(cache.inner_vertices.get().is_some(), expected);
    assert_eq!(cache.outer_vertices.get().is_some(), expected);
    assert_eq!(cache.inner_faces.get().is_some(), expected);
    assert_eq!(cache.outer_faces.get().is_some(), expected);
}

#[test]
fn boundary_cache_invalidates_on_selection_edits() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    populate_boundary_cache(&mesh);

    mesh.selection_mut().select(&verts[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 1);

    mesh.selection_mut().grow();
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 3);

    mesh.selection_mut().shrink();
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);

    mesh.selection_mut().set(&verts[..2]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 2);

    mesh.selection_mut().deselect(&verts[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection().boundary().vertices().collect::<Vec<_>>(),
        vec![verts[1]]
    );

    mesh.selection_mut().toggle(&[verts[1], verts[2]]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(
        mesh.selection().boundary().vertices().collect::<Vec<_>>(),
        vec![verts[2]]
    );

    mesh.selection_mut().clear();
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);

    mesh.selection_mut().select(&verts[..1]);
    populate_boundary_cache(&mesh);
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);
}

#[test]
fn boundary_cache_invalidates_on_cascaded_vertex_changes() {
    let mut mesh = quad_grid(2, 1);
    let faces: Vec<_> = mesh.topology.faces.keys().collect();
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    populate_boundary_cache(&mesh);

    mesh.selection_mut().select(&faces[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 2);

    mesh.selection_mut().deselect(&faces[..1]);
    assert_boundary_cache_initialized(&mesh, false);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);
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
    mesh.selection_mut().set_level(ComponentTypes::VERTEX);
    mesh.selection_mut().toggle(&[face]);
    mesh.selection_mut().select(&[VertKey::null()]);
    assert_boundary_cache_initialized(&mesh, true);

    mesh.attributes.positions[verts[0]] += Vec3::Z;
    assert_boundary_cache_initialized(&mesh, true);
    assert_eq!(
        mesh.selection().boundary().vertices().collect::<Vec<_>>(),
        vec![verts[0]]
    );
    assert_boundary_cache_initialized(&mesh, true);

    mesh.selection_mut().select(&verts);
    populate_boundary_cache(&mesh);
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_boundary_cache_initialized(&mesh, true);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);

    mesh.selection_mut().clear();
    populate_boundary_cache(&mesh);
    mesh.selection_mut().clear();
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_boundary_cache_initialized(&mesh, true);
}

#[test]
fn boundary_cache_can_be_invalidated_after_topology_edits() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().select(&verts[..1]);
    populate_boundary_cache(&mesh);
    assert_eq!(mesh.selection().boundary().outer_vertices().count(), 2);

    mesh.topology.insert_edge([verts[0], verts[3]]);
    mesh.selection.invalidate_boundary();
    assert_boundary_cache_initialized(&mesh, false);
    assert_eq!(mesh.selection().boundary().outer_vertices().count(), 3);
}

#[test]
fn grow_adds_one_edge_ring_at_a_time() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert!(mesh.selection().is_empty());

    mesh.selection_mut().select(&[verts[12]]);
    mesh.selection_mut().grow();
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        [7, 11, 12, 13, 17].map(|index| verts[index]).into()
    );
    assert_eq!(mesh.selection().edges().count(), 4);
    assert_eq!(mesh.selection().faces().count(), 0);

    for count in [13, 21, 25] {
        mesh.selection_mut().grow();
        assert_eq!(mesh.selection().verts().count(), count);
    }
    assert_eq!(mesh.selection().faces().count(), 16);
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_eq!(mesh.selection().verts().count(), 25);
    assert_eq!(mesh.selection().faces().count(), 16);
    assert_eq!(mesh.selection().level(), ComponentTypes::VERTEX);
}

#[test]
fn shrink_removes_the_boundary_and_recomputes_it() {
    let mut mesh = quad_grid(4, 4);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let selected = [6, 7, 8, 11, 12, 13, 16, 17, 18].map(|index| verts[index]);
    mesh.selection_mut().select(&selected);
    mesh.selection_mut().shrink();
    assert_eq!(
        mesh.selection().verts().collect::<Vec<_>>(),
        vec![verts[12]]
    );
    assert_eq!(mesh.selection().edges().count(), 0);
    assert_eq!(mesh.selection().faces().count(), 0);
    assert_eq!(mesh.selection().boundary().vertices().count(), 1);
    mesh.selection_mut().shrink();
    assert!(mesh.selection().is_empty());
    mesh.selection_mut().shrink();
    assert!(mesh.selection().is_empty());
}

#[test]
fn boundary_of_empty_partial_and_full_selection() {
    let mut mesh = triangle();
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let face = mesh.topology.faces.keys().next().unwrap();

    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.vertices().count(), 0);
    assert_eq!(boundary.inner_vertices().count(), 0);
    assert_eq!(boundary.outer_vertices().count(), 0);
    assert_eq!(boundary.inner_faces().count(), 0);
    assert_eq!(boundary.outer_faces().count(), 0);

    mesh.selection_mut().select(&verts[..1]);
    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.vertices().collect::<Vec<_>>(), vec![verts[0]]);
    assert_eq!(boundary.inner_vertices().count(), 0);
    assert_eq!(boundary.inner_faces().count(), 0);
    assert_eq!(
        boundary.outer_vertices().collect::<HashSet<_>>(),
        HashSet::from([verts[1], verts[2]])
    );
    assert_eq!(boundary.outer_faces().collect::<Vec<_>>(), vec![face]);
    assert_eq!(mesh.selection().len(), 1);

    mesh.selection_mut().select(&verts[1..]);
    let boundary = mesh.selection().boundary();
    for _ in 0..2 {
        assert_eq!(boundary.vertices().count(), 0);
        assert_eq!(boundary.inner_vertices().count(), 0);
        assert_eq!(boundary.outer_vertices().count(), 0);
        assert_eq!(boundary.inner_faces().count(), 0);
        assert_eq!(boundary.outer_faces().count(), 0);
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
    let outer_faces: HashSet<_> = faces
        .iter()
        .copied()
        .filter(|key| !inner_faces.contains(key))
        .collect();

    for _ in 0..2 {
        assert_eq!(boundary.outer_faces().collect::<HashSet<_>>(), outer_faces);
        assert_eq!(boundary.inner_faces().collect::<HashSet<_>>(), inner_faces);
        assert_eq!(
            boundary.vertices().collect::<HashSet<_>>(),
            [6, 7, 8, 11, 13, 16, 17, 18]
                .map(|index| verts[index])
                .into()
        );
        assert_eq!(
            boundary.inner_vertices().collect::<Vec<_>>(),
            vec![verts[12]]
        );
        assert_eq!(
            boundary.outer_vertices().collect::<HashSet<_>>(),
            [1, 2, 3, 5, 9, 10, 14, 15, 19, 21, 22, 23]
                .map(|index| verts[index])
                .into()
        );
        assert_eq!(boundary.outer_faces().count(), 12);
        assert_eq!(boundary.inner_faces().count(), 4);
        assert_eq!(boundary.vertices().count(), 8);
        assert_eq!(boundary.outer_vertices().count(), 12);
    }
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        selected.into()
    );
}

#[test]
fn boundary_uses_faces_but_rings_use_edges_on_polygons() {
    let mut mesh = quad_grid(1, 1);
    let verts: Vec<_> = mesh.topology.verts.keys().collect();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection_mut().select(&verts[..1]);
    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.inner_faces().count(), 0);
    assert_eq!(boundary.outer_faces().collect::<Vec<_>>(), vec![face]);
    assert_eq!(
        boundary.outer_vertices().collect::<HashSet<_>>(),
        HashSet::from([verts[1], verts[2]])
    );

    mesh.selection_mut().grow();
    let boundary = mesh.selection().boundary();
    assert_eq!(
        boundary.vertices().collect::<HashSet<_>>(),
        HashSet::from([verts[0], verts[1], verts[2]])
    );
    assert_eq!(boundary.inner_vertices().count(), 0);
    assert_eq!(
        boundary.outer_vertices().collect::<Vec<_>>(),
        vec![verts[3]]
    );
    mesh.selection_mut().shrink();
    assert!(mesh.selection().is_empty());
}

#[test]
fn boundary_only_faces_belong_to_neither_face_ring() {
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
    assert_eq!(boundary.inner_vertices().count(), 0);
    assert_eq!(boundary.inner_faces().count(), 0);
    assert_eq!(
        boundary.outer_faces().collect::<HashSet<_>>(),
        faces[1..].iter().copied().collect()
    );
    assert_eq!(boundary.vertices().count(), 3);
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
    assert_eq!(
        boundary.vertices().collect::<HashSet<_>>(),
        HashSet::from([verts[0], verts[1]])
    );
    assert_eq!(
        boundary.inner_vertices().collect::<Vec<_>>(),
        vec![verts[2]]
    );
    assert_eq!(
        boundary.outer_vertices().collect::<HashSet<_>>(),
        HashSet::from([verts[3], verts[4], verts[5]])
    );
    assert_eq!(boundary.inner_faces().collect::<Vec<_>>(), vec![faces[0]]);
    assert_eq!(
        boundary.outer_faces().collect::<HashSet<_>>(),
        HashSet::from([faces[1], faces[2]])
    );
    assert_eq!(boundary.outer_faces().count(), 2);

    mesh.selection_mut().shrink();
    assert_eq!(
        mesh.selection().verts().collect::<HashSet<_>>(),
        HashSet::from([verts[2], verts[6]])
    );
    mesh.selection_mut().set(&verts[..5]);
    assert_eq!(mesh.selection().boundary().vertices().count(), 0);
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_eq!(mesh.selection().verts().count(), 5);
    assert!(!mesh.selection().contains(verts[5]));

    mesh.selection_mut().set(&verts[5..]);
    let boundary = mesh.selection().boundary();
    assert_eq!(boundary.vertices().count(), 0);
    assert_eq!(boundary.inner_vertices().count(), 0);
    assert_eq!(boundary.outer_vertices().count(), 0);
    assert_eq!(boundary.inner_faces().count(), 0);
    assert_eq!(boundary.outer_faces().count(), 0);
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_eq!(mesh.selection().verts().count(), 2);
}

#[test]
fn grow_preserves_each_selection_mode_and_propagates_components() {
    for bits in 1..=ComponentTypes::all().bits() {
        let level = ComponentTypes::from_bits_retain(bits);
        let mut mesh = quad_grid(3, 3);
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        mesh.selection_mut().set_level(ComponentTypes::FACE);
        mesh.selection_mut().select(&[faces[4]]);
        mesh.selection_mut().set_level(level);
        mesh.selection_mut().grow();
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
fn shrink_restores_each_mode_and_discards_unsupported_remnants() {
    for bits in 0..=ComponentTypes::all().bits() {
        let level = ComponentTypes::from_bits_retain(bits);
        let mut mesh = quad_grid(4, 4);
        let verts: Vec<_> = mesh.topology.verts.keys().collect();
        let faces: Vec<_> = mesh.topology.faces.keys().collect();
        mesh.selection_mut().set_level(ComponentTypes::FACE);
        mesh.selection_mut()
            .select(&[faces[5], faces[6], faces[9], faces[10]]);
        mesh.selection_mut().set_level(level);
        mesh.selection_mut().shrink();
        assert_eq!(mesh.selection().level(), level);
        assert_eq!(mesh.selection().edges().count(), 0, "{level:?}");
        assert_eq!(mesh.selection().faces().count(), 0, "{level:?}");
        if level.contains(ComponentTypes::VERTEX) {
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
fn boundary_operations_without_a_boundary_do_not_rebuild_selection() {
    let mut mesh = triangle();
    let edges: Vec<_> = mesh.topology.edges.keys().collect();
    mesh.selection_mut().set_level(ComponentTypes::EDGE);
    mesh.selection_mut().select(&edges[..2]);
    mesh.selection_mut().grow();
    mesh.selection_mut().shrink();
    assert_eq!(mesh.selection().level(), ComponentTypes::EDGE);
    assert_eq!(mesh.selection().verts().count(), 3);
    assert_eq!(mesh.selection().edges().count(), 2);
    assert_eq!(mesh.selection().faces().count(), 0);
}

#[test]
fn accessors_expose_each_selected_kind() {
    let mut mesh = triangle();
    assert_eq!(mesh.selection().level(), ComponentTypes::VERTEX);
    assert!(mesh.selection().is_empty());
    let vert = mesh.topology.verts.keys().next().unwrap();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let face = mesh.topology.faces.keys().next().unwrap();
    mesh.selection.verts.insert(vert);
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
    mesh.selection_mut().set_level(ComponentTypes::EDGE);
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
    mesh.selection_mut().set_level(ComponentTypes::FACE);
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
        .set_level(ComponentTypes::VERTEX | ComponentTypes::EDGE);
    mesh.selection_mut()
        .select(&[ComponentKey::Face(face), edge.into()]);
    assert_eq!(mesh.selection().len(), 3);
    mesh.selection_mut().select(&verts);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().deselect(&verts);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set_level(ComponentTypes::empty());
    assert!(mesh.selection().is_empty());
    mesh.selection_mut().select(&[face]);
    assert!(mesh.selection().is_empty());
}

#[test]
fn toggle_deduplicates_and_uses_pre_cascade_membership() {
    let mut mesh = triangle();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let verts = mesh.topology.edges[edge].verts;
    mesh.selection_mut().set_level(ComponentTypes::all());
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
    let mode = ComponentTypes::VERTEX | ComponentTypes::EDGE;
    mesh.selection_mut().set_level(mode);
    mesh.selection_mut().select(&edges[..2]);
    mesh.selection_mut().select(&verts);
    mesh.selection_mut().select(&edges[..2]);
    mesh.selection_mut().deselect(&edges[2..]);
    mesh.selection_mut().set_level(mode);
    assert_eq!(mesh.selection().len(), 5);
    assert!(!mesh.selection().contains(edges[2]));

    mesh.selection_mut().set_level(ComponentTypes::VERTEX);
    assert_eq!(mesh.selection().len(), 7);
    mesh.selection_mut().set(&verts[..1]);
    mesh.selection_mut().set_level(ComponentTypes::FACE);
    assert!(mesh.selection().is_empty());
}

#[test]
fn mixed_toggle_processes_removals_before_additions() {
    let mut mesh = triangle();
    let edge = mesh.topology.edges.keys().next().unwrap();
    let verts = mesh.topology.edges[edge].verts;
    mesh.selection_mut().set_level(ComponentTypes::all());
    mesh.selection_mut().select(&verts[..1]);
    mesh.selection_mut()
        .toggle(&[ComponentKey::Vert(verts[0]), edge.into()]);
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
    mesh.selection_mut().set_level(ComponentTypes::all());
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
    mesh.selection_mut().set_level(ComponentTypes::FACE);
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
    mesh.selection_mut().set_level(ComponentTypes::all());
    mesh.selection_mut().select(&missing);
    mesh.selection_mut().toggle(&missing);
    mesh.selection_mut().deselect(&missing);
    assert!(mesh.selection().is_empty());
}
