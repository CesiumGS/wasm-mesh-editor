use super::*;

#[test]
fn component_types_convert_individual_kinds() {
    for (kind, expected) in [
        (ComponentType::Vertex, ComponentTypes::VERTEX),
        (ComponentType::Edge, ComponentTypes::EDGE),
        (ComponentType::Face, ComponentTypes::FACE),
    ] {
        assert_eq!(ComponentTypes::from(kind), expected);
    }
}

#[test]
fn component_keys_index_independent_kind_values() {
    use slotmap::Key;

    let keys = [
        ComponentKey::Vert(VertKey::null()),
        ComponentKey::Edge(EdgeKey::null()),
        ComponentKey::Face(FaceKey::null()),
    ];
    let kinds = [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ];
    let mut values = PerComponentType::<Vec<ComponentKey>>::default();
    for (key, kind) in keys.into_iter().zip(kinds) {
        assert_eq!(key.kind(), kind);
        values[key.kind()].push(key);
    }
    let reader = &values;
    for (key, kind) in keys.into_iter().zip(kinds) {
        assert_eq!(reader[kind], vec![key]);
    }
}
