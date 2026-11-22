use super::*;
use crate::{
    AttributeBuffer, ComponentKeys, ComponentType, EdgeKey, FaceKey, Mesh, MeshBuffers, VertKey,
};
use event_emitter::Event;
use glam::Vec3;
use slotmap::Key;
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, TryRecvError};

/// Two triangles sharing the edge between vertices 0 and 2: faces `[0, 1, 2]` and `[0, 2, 3]`.
/// Each vertex's `_ID` attribute is its index.
struct TwoTriangles {
    mesh: Mesh,
    verts: Vec<VertKey>,
    faces: Vec<FaceKey>,
}

impl TwoTriangles {
    fn new() -> Self {
        let mesh = Mesh::from_buffers(MeshBuffers {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            indices: vec![0, 1, 2, 0, 2, 3],
            vertex_attributes: vec![("_ID".into(), AttributeBuffer::U32(vec![0, 1, 2, 3]))],
            ..Default::default()
        })
        .unwrap();
        let verts = mesh.topology.verts().collect();
        let faces = mesh.topology.faces().collect();

        Self { mesh, verts, faces }
    }

    fn edge(&self, first: usize, second: usize) -> EdgeKey {
        let (first, second) = (self.verts[first], self.verts[second]);

        self.mesh
            .vert(first)
            .unwrap()
            .edges()
            .find(|&edge| self.mesh.edge(edge).unwrap().verts().contains(&second))
            .unwrap()
    }
}

fn keys(keys: &[ComponentKey]) -> HashSet<ComponentKey> {
    keys.iter().copied().collect()
}

fn hidden(mesh: &Mesh) -> HashSet<ComponentKey> {
    mesh.hidden_components().iter().collect()
}

fn selected(mesh: &Mesh) -> HashSet<ComponentKey> {
    mesh.selection().iter().collect()
}

fn flatten(keys: &ComponentKeys) -> HashSet<ComponentKey> {
    [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ]
    .into_iter()
    .flat_map(|kind| keys.iter(kind))
    .collect()
}

fn assert_hidden_event(
    changes: &Receiver<Event<MeshChange>>,
    added: &[ComponentKey],
    removed: &[ComponentKey],
) {
    let event = changes.try_recv().unwrap();
    let MeshChange::Hidden(change) = event.as_ref() else {
        panic!("expected a hidden change");
    };

    assert_eq!(flatten(&change.added), keys(added));
    assert_eq!(flatten(&change.removed), keys(removed));
}

fn assert_no_event(changes: &Receiver<Event<MeshChange>>) {
    assert!(matches!(changes.try_recv(), Err(TryRecvError::Empty)));
}

const EXTEND: HideOptions = HideOptions {
    extend_to_connected_faces: true,
};

#[test]
fn hide_and_show_only_accept_interactable_types() {
    let TwoTriangles {
        mut mesh,
        verts,
        faces,
    } = TwoTriangles::new();
    let edge = mesh.topology.edges().next().unwrap();
    let changes = mesh.subscribe();

    mesh.hidden_components_mut().hide(
        &[ComponentKey::Edge(edge), ComponentKey::Face(faces[0])],
        HideOptions::default(),
    );
    assert!(mesh.hidden_components().is_empty());
    assert_no_event(&changes);

    mesh.hidden_components_mut()
        .hide(&[verts[1]], HideOptions::default());
    assert_eq!(hidden(&mesh), keys(&[verts[1].into()]));
    assert_hidden_event(&changes, &[verts[1].into()], &[]);

    mesh.set_interactable_components(ComponentMask::FACE);
    mesh.hidden_components_mut()
        .show(&[verts[1]], HideOptions::default());
    assert!(mesh.hidden_components().contains(verts[1]));
    assert_no_event(&changes);

    mesh.set_interactable_components(ComponentMask::VERTEX);
    mesh.hidden_components_mut()
        .show(&[verts[1]], HideOptions::default());
    assert!(mesh.hidden_components().is_empty());
    assert_hidden_event(&changes, &[], &[verts[1].into()]);
}

#[test]
fn hide_ignores_missing_keys() {
    let mut mesh = TwoTriangles::new().mesh;
    let changes = mesh.subscribe();

    mesh.hidden_components_mut()
        .hide(&[VertKey::null()], EXTEND);

    assert!(mesh.hidden_components().is_empty());
    assert_no_event(&changes);
}

#[test]
fn hiding_does_not_affect_other_components_by_default() {
    let TwoTriangles {
        mut mesh, verts, ..
    } = TwoTriangles::new();

    mesh.hidden_components_mut()
        .hide(&[verts[0]], HideOptions::default());

    assert_eq!(hidden(&mesh), keys(&[verts[0].into()]));
}

#[test]
fn extending_hides_faces_using_each_component() {
    let fixture = TwoTriangles::new();
    let (verts, faces) = (&fixture.verts, &fixture.faces);
    let shared_edge = fixture.edge(0, 2);
    let outer_edge = fixture.edge(0, 1);
    let cases = [
        (ComponentMask::VERTEX, verts[1].into(), vec![faces[0]]),
        (ComponentMask::VERTEX, verts[0].into(), faces.clone()),
        (ComponentMask::EDGE, shared_edge.into(), faces.clone()),
        (ComponentMask::EDGE, outer_edge.into(), vec![faces[0]]),
        (ComponentMask::FACE, faces[0].into(), vec![faces[0]]),
    ];

    for (interactable, key, expected_faces) in cases {
        let mut mesh = TwoTriangles::new().mesh;
        mesh.set_interactable_components(interactable);

        mesh.hidden_components_mut().hide(&[key], EXTEND);

        let expected: HashSet<_> = expected_faces
            .into_iter()
            .map(ComponentKey::Face)
            .chain([key])
            .collect();
        assert_eq!(hidden(&mesh), expected, "hiding {key:?}");
    }
}

#[test]
fn extending_shows_faces_even_if_the_component_was_visible() {
    let TwoTriangles {
        mut mesh,
        verts,
        faces,
    } = TwoTriangles::new();
    mesh.hidden_components_mut().hide(&[verts[0]], EXTEND);
    let changes = mesh.subscribe();

    mesh.hidden_components_mut().show(&[verts[1]], EXTEND);

    assert_eq!(hidden(&mesh), keys(&[verts[0].into(), faces[1].into()]));
    assert_hidden_event(&changes, &[], &[faces[0].into()]);
}

#[test]
fn show_all_shows_every_type_regardless_of_interactable_types() {
    let TwoTriangles {
        mut mesh,
        verts,
        faces,
    } = TwoTriangles::new();
    mesh.hidden_components_mut().hide(&[verts[0]], EXTEND);
    mesh.set_interactable_components(ComponentMask::empty());
    let changes = mesh.subscribe();

    mesh.hidden_components_mut().show_all();

    assert!(mesh.hidden_components().is_empty());
    assert_hidden_event(
        &changes,
        &[],
        &[verts[0].into(), faces[0].into(), faces[1].into()],
    );

    mesh.hidden_components_mut().show_all();
    assert_no_event(&changes);
}

#[test]
fn events_contain_only_net_changes() {
    let TwoTriangles {
        mut mesh, verts, ..
    } = TwoTriangles::new();
    let changes = mesh.subscribe();

    mesh.hidden_components_mut()
        .hide(&[verts[0]], HideOptions::default());
    assert_hidden_event(&changes, &[verts[0].into()], &[]);

    mesh.hidden_components_mut()
        .hide(&[verts[0]], HideOptions::default());
    assert_no_event(&changes);

    mesh.hidden_components_mut()
        .hide(&[verts[0], verts[1], verts[1]], HideOptions::default());
    assert_hidden_event(&changes, &[verts[1].into()], &[]);

    mesh.hidden_components_mut()
        .show(&[verts[2]], HideOptions::default());
    assert_no_event(&changes);
}

#[test]
fn hide_and_show_do_not_change_selection() {
    let TwoTriangles {
        mut mesh, verts, ..
    } = TwoTriangles::new();
    mesh.selection_mut().select(&[verts[0]]);
    let changes = mesh.subscribe();

    mesh.hidden_components_mut().hide(&[verts[0]], EXTEND);
    mesh.hidden_components_mut().show(&[verts[0]], EXTEND);

    assert_eq!(selected(&mesh), keys(&[verts[0].into()]));
    assert!(matches!(
        changes.try_recv().unwrap().as_ref(),
        MeshChange::Hidden(_)
    ));
    assert!(matches!(
        changes.try_recv().unwrap().as_ref(),
        MeshChange::Hidden(_)
    ));
    assert_no_event(&changes);
}

#[test]
fn hide_and_show_verts_by_attribute() {
    let TwoTriangles {
        mut mesh,
        verts,
        faces,
    } = TwoTriangles::new();
    let changes = mesh.subscribe();

    mesh.hidden_components_mut()
        .hide_by_attribute::<VertKey, _>("_ID", |id: u32| id >= 2, HideOptions::default())
        .unwrap();
    assert_eq!(hidden(&mesh), keys(&[verts[2].into(), verts[3].into()]));
    assert_hidden_event(&changes, &[verts[2].into(), verts[3].into()], &[]);

    mesh.hidden_components_mut()
        .hide_by_attribute::<VertKey, _>("_ID", |id: u32| id == 1, EXTEND)
        .unwrap();
    assert_eq!(
        hidden(&mesh),
        keys(&[
            verts[1].into(),
            verts[2].into(),
            verts[3].into(),
            faces[0].into()
        ])
    );

    mesh.hidden_components_mut()
        .show_by_attribute::<VertKey, _>("_ID", |id: u32| id != 3, EXTEND)
        .unwrap();
    assert_eq!(hidden(&mesh), keys(&[verts[3].into()]));
}

#[test]
fn hiding_by_attribute_only_hides_interactable_vertices() {
    let mut mesh = TwoTriangles::new().mesh;
    mesh.set_interactable_components(ComponentMask::FACE);
    let changes = mesh.subscribe();

    mesh.hidden_components_mut()
        .hide_by_attribute::<VertKey, _>("_ID", |_: u32| true, EXTEND)
        .unwrap();

    assert!(mesh.hidden_components().is_empty());
    assert_no_event(&changes);
}

#[test]
fn hiding_by_attribute_reports_lookup_errors() {
    let mut mesh = TwoTriangles::new().mesh;
    let mut hidden = mesh.hidden_components_mut();

    assert_eq!(
        hidden.hide_by_attribute::<VertKey, _>("_MISSING", |_: u32| true, HideOptions::default()),
        Err(AttributeError::NotFound {
            name: "_MISSING".into()
        })
    );
    assert_eq!(
        hidden.show_by_attribute::<VertKey, _>("_ID", |_: f32| true, HideOptions::default()),
        Err(AttributeError::WrongType { name: "_ID".into() })
    );
}
