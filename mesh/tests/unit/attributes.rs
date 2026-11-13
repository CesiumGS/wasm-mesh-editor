use crate::{FaceKey, Mesh, MeshBuffers};

use super::*;

/// Two triangles sharing the edge between vertices 1 and 2.
fn two_triangles() -> Mesh {
    Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ONE],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 2, 1, 3],
        face_vertex_counts: None,
    })
    .unwrap()
}

fn keys(mesh: &Mesh) -> (Vec<VertKey>, Vec<FaceKey>) {
    (
        mesh.topology.verts().collect(),
        mesh.topology.faces().collect(),
    )
}

/// Whether `typed::<T>` returns values, for each value type, in the order F32, Vec2, Vec3, Vec4, U32, I32.
fn typed_matches(map: &AttributeMap<VertKey>) -> [bool; 6] {
    [
        map.typed::<f32>().is_some(),
        map.typed::<Vec2>().is_some(),
        map.typed::<Vec3>().is_some(),
        map.typed::<Vec4>().is_some(),
        map.typed::<u32>().is_some(),
        map.typed::<i32>().is_some(),
    ]
}

#[test]
fn new_attributes_have_no_custom_attributes() {
    let attributes = Attributes::default();

    assert_eq!(attributes.vertex_attributes().count(), 0);
    assert_eq!(attributes.corner_attributes().count(), 0);
    assert!(attributes.vertex_attribute("_FEATURE_ID_0").is_none());
    assert!(attributes.corner_attribute("_FEATURE_ID_0").is_none());

    let attributes = Attributes::with_capacity(4, 4, 0);
    assert_eq!(attributes.vertex_attributes().count(), 0);
    assert_eq!(attributes.corner_attributes().count(), 0);
}

#[test]
fn typed_returns_values_only_for_the_stored_type() {
    let maps = [
        AttributeMap::F32(SecondaryMap::new()),
        AttributeMap::Vec2(SecondaryMap::new()),
        AttributeMap::Vec3(SecondaryMap::new()),
        AttributeMap::Vec4(SecondaryMap::new()),
        AttributeMap::U32(SecondaryMap::new()),
        AttributeMap::I32(SecondaryMap::new()),
    ];

    for (variant, map) in maps.iter().enumerate() {
        let mut expected = [false; 6];
        expected[variant] = true;
        assert_eq!(typed_matches(map), expected, "variant {variant}");
    }
}

#[test]
fn lookups_find_attributes_by_name_within_their_domain() {
    let mut attributes = Attributes::default();
    attributes.add_vertex_attribute("_B".into(), AttributeMap::U32(SecondaryMap::new()));
    attributes.add_corner_attribute("_C".into(), AttributeMap::Vec4(SecondaryMap::new()));
    attributes.add_vertex_attribute("_A".into(), AttributeMap::F32(SecondaryMap::new()));

    assert_eq!(
        attributes
            .vertex_attributes()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["_B", "_A"]
    );
    assert_eq!(
        attributes
            .corner_attributes()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["_C"]
    );

    assert!(matches!(
        attributes.vertex_attribute("_A"),
        Some(AttributeMap::F32(_))
    ));
    assert!(matches!(
        attributes.vertex_attribute("_B"),
        Some(AttributeMap::U32(_))
    ));
    assert!(matches!(
        attributes.corner_attribute("_C"),
        Some(AttributeMap::Vec4(_))
    ));

    assert!(attributes.vertex_attribute("_C").is_none());
    assert!(attributes.corner_attribute("_A").is_none());
    assert!(attributes.vertex_attribute("_a").is_none());
    assert!(attributes.vertex_attribute("_MISSING").is_none());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "duplicate custom attribute")]
fn names_are_unique_across_domains() {
    let mut attributes = Attributes::default();
    attributes.add_vertex_attribute("_A".into(), AttributeMap::U32(SecondaryMap::new()));
    attributes.add_corner_attribute("_A".into(), AttributeMap::U32(SecondaryMap::new()));
}

#[test]
fn vertex_views_read_typed_values() {
    let mut mesh = two_triangles();
    let (verts, faces) = keys(&mesh);

    let mut ids = SecondaryMap::new();
    for (index, &vert) in verts.iter().take(3).enumerate() {
        ids.insert(vert, index as u32 + 10);
    }
    mesh.attributes
        .add_vertex_attribute("_FEATURE_ID_0".into(), AttributeMap::U32(ids));
    mesh.attributes
        .add_corner_attribute("_CORNER".into(), AttributeMap::U32(SecondaryMap::new()));

    for (index, &vert) in verts.iter().take(3).enumerate() {
        let view = mesh.vert(vert).unwrap();
        assert_eq!(
            view.attribute::<u32>("_FEATURE_ID_0"),
            Some(index as u32 + 10)
        );
    }

    let view = mesh.vert(verts[0]).unwrap();
    assert_eq!(view.attribute::<i32>("_FEATURE_ID_0"), None);
    assert_eq!(view.attribute::<f32>("_FEATURE_ID_0"), None);
    assert_eq!(view.attribute::<u32>("_MISSING"), None);
    assert_eq!(view.attribute::<u32>("_CORNER"), None);
    assert_eq!(
        mesh.face(faces[0])
            .unwrap()
            .corner_attribute::<u32>("_FEATURE_ID_0", verts[0]),
        None
    );

    let unset = mesh.vert(verts[3]).unwrap();
    assert_eq!(unset.attribute::<u32>("_FEATURE_ID_0"), None);
}

#[test]
fn corner_values_can_differ_at_a_shared_vertex() {
    let mut mesh = two_triangles();
    let (verts, faces) = keys(&mesh);

    // Each corner's value identifies its face and vertex.
    let mut colors = SecondaryMap::new();
    for (face_index, &face) in faces.iter().enumerate() {
        for loop_key in mesh.topology.face_loops(face) {
            let vert = mesh.topology.loops[loop_key].vert;
            let vert_index = verts.iter().position(|&key| key == vert).unwrap();
            colors.insert(
                loop_key,
                Vec4::new(face_index as f32, vert_index as f32, 0.0, 1.0),
            );
        }
    }
    mesh.attributes
        .add_corner_attribute("_COLOR".into(), AttributeMap::Vec4(colors));

    let first = mesh.face(faces[0]).unwrap();
    let second = mesh.face(faces[1]).unwrap();

    for shared in [1, 2] {
        assert_eq!(
            first.corner_attribute::<Vec4>("_COLOR", verts[shared]),
            Some(Vec4::new(0.0, shared as f32, 0.0, 1.0))
        );
        assert_eq!(
            second.corner_attribute::<Vec4>("_COLOR", verts[shared]),
            Some(Vec4::new(1.0, shared as f32, 0.0, 1.0))
        );
    }

    assert_eq!(first.corner_attribute::<Vec4>("_COLOR", verts[3]), None);
    assert_eq!(second.corner_attribute::<Vec4>("_COLOR", verts[0]), None);
    assert_eq!(first.corner_attribute::<Vec3>("_COLOR", verts[0]), None);
    assert_eq!(first.corner_attribute::<Vec4>("_MISSING", verts[0]), None);
    assert_eq!(
        mesh.vert(verts[1]).unwrap().attribute::<Vec4>("_COLOR"),
        None
    );
}
