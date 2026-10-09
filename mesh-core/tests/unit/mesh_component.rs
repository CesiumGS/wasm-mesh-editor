use super::*;

#[test]
fn component_mask_converts_individual_kinds() {
    for (kind, expected) in [
        (ComponentType::Vertex, ComponentMask::VERTEX),
        (ComponentType::Edge, ComponentMask::EDGE),
        (ComponentType::Face, ComponentMask::FACE),
    ] {
        assert_eq!(ComponentMask::from(kind), expected);
    }
}

#[test]
fn component_keys_report_their_kind() {
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
    for (key, kind) in keys.into_iter().zip(kinds) {
        assert_eq!(key.kind(), kind);
    }
}
