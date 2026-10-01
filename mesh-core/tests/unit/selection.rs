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
