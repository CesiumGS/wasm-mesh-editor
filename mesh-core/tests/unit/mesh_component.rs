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
