use crate::{EdgeKey, FaceKey, Mesh, MeshBuffers};

use super::*;

/// Two triangles sharing the edge between vertices 1 and 2.
fn two_triangles() -> Mesh {
    Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ONE],
        normals: None,
        uvs: None,
        indices: vec![0, 1, 2, 2, 1, 3],
        face_vertex_counts: None,
        ..Default::default()
    })
    .unwrap()
}

fn keys(mesh: &Mesh) -> (Vec<VertKey>, Vec<FaceKey>) {
    (
        mesh.topology.verts().collect(),
        mesh.topology.faces().collect(),
    )
}

/// Whether `typed::<T>` returns values, for each value type, in [`AttributeMap`] variant order.
fn typed_matches(map: &AttributeMap<VertKey>) -> [bool; 12] {
    [
        map.typed::<f32>().is_some(),
        map.typed::<Vec2>().is_some(),
        map.typed::<Vec3>().is_some(),
        map.typed::<Vec4>().is_some(),
        map.typed::<u32>().is_some(),
        map.typed::<UVec2>().is_some(),
        map.typed::<UVec3>().is_some(),
        map.typed::<UVec4>().is_some(),
        map.typed::<i32>().is_some(),
        map.typed::<IVec2>().is_some(),
        map.typed::<IVec3>().is_some(),
        map.typed::<IVec4>().is_some(),
    ]
}

#[test]
fn new_attributes_have_no_custom_attributes() {
    let attributes = Attributes::default();

    assert_eq!(attributes.custom_attributes::<VertKey>().count(), 0);
    assert_eq!(attributes.custom_attributes::<LoopKey>().count(), 0);
    assert!(
        attributes
            .custom_attribute::<VertKey>("_FEATURE_ID_0")
            .is_none()
    );
    assert!(
        attributes
            .custom_attribute::<LoopKey>("_FEATURE_ID_0")
            .is_none()
    );

    let attributes = Attributes::with_capacity(4, 4, 0);
    assert_eq!(attributes.custom_attributes::<VertKey>().count(), 0);
    assert_eq!(attributes.custom_attributes::<LoopKey>().count(), 0);
}

#[test]
fn typed_returns_values_only_for_the_stored_type() {
    let maps = [
        AttributeMap::F32(SecondaryMap::new()),
        AttributeMap::Vec2(SecondaryMap::new()),
        AttributeMap::Vec3(SecondaryMap::new()),
        AttributeMap::Vec4(SecondaryMap::new()),
        AttributeMap::U32(SecondaryMap::new()),
        AttributeMap::UVec2(SecondaryMap::new()),
        AttributeMap::UVec3(SecondaryMap::new()),
        AttributeMap::UVec4(SecondaryMap::new()),
        AttributeMap::I32(SecondaryMap::new()),
        AttributeMap::IVec2(SecondaryMap::new()),
        AttributeMap::IVec3(SecondaryMap::new()),
        AttributeMap::IVec4(SecondaryMap::new()),
    ];

    for (variant, map) in maps.iter().enumerate() {
        let mut expected = [false; 12];
        expected[variant] = true;
        assert_eq!(typed_matches(map), expected, "variant {variant}");
    }
}

#[test]
fn lookups_find_attributes_by_name_within_their_domain() {
    let mut attributes = Attributes::default();
    attributes.add_custom::<VertKey>("_B".into(), AttributeMap::U32(SecondaryMap::new()));
    attributes.add_custom::<LoopKey>("_C".into(), AttributeMap::Vec4(SecondaryMap::new()));
    attributes.add_custom::<VertKey>("_A".into(), AttributeMap::F32(SecondaryMap::new()));

    assert_eq!(
        attributes
            .custom_attributes::<VertKey>()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["_B", "_A"]
    );
    assert_eq!(
        attributes
            .custom_attributes::<LoopKey>()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["_C"]
    );

    assert!(matches!(
        attributes.custom_attribute::<VertKey>("_A"),
        Some(AttributeMap::F32(_))
    ));
    assert!(matches!(
        attributes.custom_attribute::<VertKey>("_B"),
        Some(AttributeMap::U32(_))
    ));
    assert!(matches!(
        attributes.custom_attribute::<LoopKey>("_C"),
        Some(AttributeMap::Vec4(_))
    ));

    assert!(attributes.custom_attribute::<VertKey>("_C").is_none());
    assert!(attributes.custom_attribute::<LoopKey>("_A").is_none());
    assert!(attributes.custom_attribute::<VertKey>("_a").is_none());
    assert!(attributes.custom_attribute::<VertKey>("_MISSING").is_none());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "duplicate custom attribute")]
fn names_are_unique_across_domains() {
    let mut attributes = Attributes::default();
    attributes.add_custom::<VertKey>("_A".into(), AttributeMap::U32(SecondaryMap::new()));
    attributes.add_custom::<LoopKey>("_A".into(), AttributeMap::U32(SecondaryMap::new()));
}

#[test]
fn value_type_reports_component_type_and_count() {
    use AttributeComponentType::{Float32, Int32, Uint32};

    let cases = [
        (AttributeMap::F32(SecondaryMap::new()), Float32, 1),
        (AttributeMap::Vec2(SecondaryMap::new()), Float32, 2),
        (AttributeMap::Vec3(SecondaryMap::new()), Float32, 3),
        (AttributeMap::Vec4(SecondaryMap::new()), Float32, 4),
        (AttributeMap::U32(SecondaryMap::new()), Uint32, 1),
        (AttributeMap::UVec2(SecondaryMap::new()), Uint32, 2),
        (AttributeMap::UVec3(SecondaryMap::new()), Uint32, 3),
        (AttributeMap::UVec4(SecondaryMap::new()), Uint32, 4),
        (AttributeMap::I32(SecondaryMap::new()), Int32, 1),
        (AttributeMap::IVec2(SecondaryMap::new()), Int32, 2),
        (AttributeMap::IVec3(SecondaryMap::new()), Int32, 3),
        (AttributeMap::IVec4(SecondaryMap::new()), Int32, 4),
    ];

    for (map, component_type, component_count) in cases {
        let map: AttributeMap<VertKey> = map;
        assert_eq!(
            map.value_type(),
            AttributeType {
                component_type,
                component_count,
            }
        );
    }
}

fn descriptor(
    name: &str,
    domain: AttributeDomain,
    component_type: AttributeComponentType,
    component_count: u8,
) -> AttributeDescriptor<'_> {
    AttributeDescriptor {
        name,
        domain,
        value_type: AttributeType {
            component_type,
            component_count,
        },
    }
}

#[test]
fn descriptors_list_built_in_then_custom_attributes() {
    use AttributeComponentType::{Float32, Int32, Uint32};
    use AttributeDomain::{Corner, Edge, Face, Vertex};

    let mut mesh = two_triangles();
    assert_eq!(
        mesh.attributes.descriptors().collect::<Vec<_>>(),
        [
            descriptor("POSITION", Vertex, Float32, 3),
            descriptor("NORMAL", Corner, Float32, 3),
        ]
    );

    mesh.attributes
        .add_custom::<LoopKey>("_COLOR".into(), AttributeMap::Vec4(SecondaryMap::new()));
    mesh.attributes
        .add_custom::<VertKey>("_B".into(), AttributeMap::U32(SecondaryMap::new()));
    mesh.attributes
        .add_custom::<VertKey>("_A".into(), AttributeMap::IVec2(SecondaryMap::new()));
    mesh.attributes
        .add_custom::<FaceKey>("_FACE".into(), AttributeMap::U32(SecondaryMap::new()));
    mesh.attributes
        .add_custom::<EdgeKey>("_EDGE".into(), AttributeMap::F32(SecondaryMap::new()));

    assert_eq!(
        mesh.attributes.descriptors().collect::<Vec<_>>(),
        [
            descriptor("POSITION", Vertex, Float32, 3),
            descriptor("NORMAL", Corner, Float32, 3),
            descriptor("_B", Vertex, Uint32, 1),
            descriptor("_A", Vertex, Int32, 2),
            descriptor("_EDGE", Edge, Float32, 1),
            descriptor("_FACE", Face, Uint32, 1),
            descriptor("_COLOR", Corner, Float32, 4),
        ]
    );
}

#[test]
fn descriptors_include_uvs_when_the_mesh_has_them() {
    use AttributeComponentType::Float32;
    use AttributeDomain::{Corner, Vertex};

    let mesh = Mesh::from_buffers(MeshBuffers {
        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        uvs: Some(vec![Vec2::ZERO, Vec2::X, Vec2::Y]),
        indices: vec![0, 1, 2],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        mesh.attributes.descriptors().collect::<Vec<_>>(),
        [
            descriptor("POSITION", Vertex, Float32, 3),
            descriptor("NORMAL", Corner, Float32, 3),
            descriptor("TEXCOORD_0", Corner, Float32, 2),
        ]
    );
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
        .add_custom::<VertKey>("_FEATURE_ID_0".into(), AttributeMap::U32(ids));
    mesh.attributes
        .add_custom::<LoopKey>("_CORNER".into(), AttributeMap::U32(SecondaryMap::new()));

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
        .add_custom::<LoopKey>("_COLOR".into(), AttributeMap::Vec4(colors));

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

#[test]
fn keys_where_returns_keys_whose_value_satisfies_the_predicate() {
    let mut mesh = two_triangles();
    let (verts, _) = keys(&mesh);
    let ids: SecondaryMap<_, _> = verts
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, &vert)| (vert, index as i32 - 1))
        .collect();
    mesh.attributes
        .add_custom::<VertKey>("_ID".into(), AttributeMap::I32(ids));
    mesh.attributes
        .add_custom::<LoopKey>("_CORNER".into(), AttributeMap::I32(SecondaryMap::new()));

    let matching = |predicate: fn(i32) -> bool| {
        let mut matching: Vec<_> = mesh
            .attributes
            .keys_where::<VertKey, _, _>("_ID", predicate)
            .unwrap()
            .collect();
        matching.sort();
        matching
    };

    // Vertex 3 has no value, so no predicate matches it.
    assert_eq!(matching(|id| id >= 0), [verts[1], verts[2]]);
    assert_eq!(matching(|id| id != 0), [verts[0], verts[2]]);
    assert_eq!(matching(|_| true), [verts[0], verts[1], verts[2]]);
    assert!(matching(|id| id > 1).is_empty());

    assert_eq!(
        mesh.attributes
            .keys_where::<VertKey, _, _>("_ID", |_: u32| true)
            .err(),
        Some(AttributeError::WrongType { name: "_ID".into() })
    );
    for name in ["_MISSING", "_CORNER"] {
        assert_eq!(
            mesh.attributes
                .keys_where::<VertKey, _, _>(name, |_: i32| true)
                .err(),
            Some(AttributeError::NotFound { name: name.into() })
        );
    }
}

#[test]
fn values_resolve_built_in_and_custom_attributes_by_domain_and_type() {
    let mut mesh = two_triangles();
    let (verts, _) = keys(&mesh);
    let ids: SecondaryMap<_, _> = verts.iter().map(|&vert| (vert, 7_u32)).collect();
    mesh.attributes
        .add_custom::<VertKey>("_ID".into(), AttributeMap::U32(ids));
    let attributes = &mesh.attributes;

    let positions = attributes.values::<VertKey, Vec3>(Attributes::POSITION);
    assert_eq!(positions, Ok(&attributes.positions));
    let normals = attributes.values::<LoopKey, Vec3>(Attributes::NORMAL);
    assert_eq!(normals, Ok(&attributes.normals));
    assert_eq!(
        attributes.values::<VertKey, u32>("_ID").unwrap()[verts[0]],
        7
    );

    let not_found = |name: &str| Some(AttributeError::NotFound { name: name.into() });
    let wrong_type = |name: &str| Some(AttributeError::WrongType { name: name.into() });
    assert_eq!(
        attributes
            .values::<LoopKey, Vec3>(Attributes::POSITION)
            .err(),
        not_found("POSITION")
    );
    assert_eq!(
        attributes
            .values::<VertKey, Vec4>(Attributes::POSITION)
            .err(),
        wrong_type("POSITION")
    );
    // The mesh has no UVs.
    assert_eq!(
        attributes
            .values::<LoopKey, Vec2>(Attributes::TEXCOORD_0)
            .err(),
        not_found("TEXCOORD_0")
    );
    assert_eq!(
        attributes.values::<LoopKey, u32>("_ID").err(),
        not_found("_ID")
    );
    assert_eq!(
        attributes.values::<VertKey, i32>("_ID").err(),
        wrong_type("_ID")
    );
}

#[test]
fn keys_where_filters_built_in_attributes() {
    let mesh = two_triangles();
    let (verts, _) = keys(&mesh);

    let mut matching: Vec<_> = mesh
        .attributes
        .keys_where::<VertKey, _, _>(Attributes::POSITION, |position: Vec3| position.x > 0.5)
        .unwrap()
        .collect();
    matching.sort();

    assert_eq!(matching, [verts[1], verts[3]]);
}

#[test]
fn descriptor_finds_built_in_and_custom_attributes() {
    let mut mesh = two_triangles();
    mesh.attributes
        .add_custom::<LoopKey>("_COLOR".into(), AttributeMap::Vec4(SecondaryMap::new()));

    let descriptor = mesh.attributes.descriptor(Attributes::POSITION).unwrap();
    assert_eq!(descriptor.domain, AttributeDomain::Vertex);
    assert_eq!(descriptor.value_type, Vec3::VALUE_TYPE);

    let descriptor = mesh.attributes.descriptor("_COLOR").unwrap();
    assert_eq!(descriptor.domain, AttributeDomain::Corner);
    assert_eq!(descriptor.value_type, Vec4::VALUE_TYPE);

    assert!(mesh.attributes.descriptor(Attributes::TEXCOORD_0).is_none());
    assert!(mesh.attributes.descriptor("_MISSING").is_none());
}
