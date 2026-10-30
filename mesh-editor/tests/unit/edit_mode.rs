use super::*;

const EDGE_AND_FACE: ComponentMask = ComponentMask::EDGE.union(ComponentMask::FACE);

#[test]
fn single_component_modes_draw_expected_components() {
    for (mode, renderable, selectable) in [
        (EditMode::NONE, ComponentMask::FACE, ComponentMask::empty()),
        (
            EditMode::VERTEX,
            ComponentMask::all(),
            ComponentMask::VERTEX,
        ),
        (EditMode::EDGE, EDGE_AND_FACE, ComponentMask::EDGE),
        (EditMode::FACE, EDGE_AND_FACE, ComponentMask::FACE),
    ] {
        assert_eq!(mode.renderable_components, renderable);
        assert_eq!(mode.selection_level, selectable);
    }
}

#[test]
fn combined_modes_draw_the_union_of_their_components() {
    for (components, renderable) in [
        (
            ComponentMask::VERTEX | ComponentMask::EDGE,
            ComponentMask::all(),
        ),
        (
            ComponentMask::VERTEX | ComponentMask::FACE,
            ComponentMask::all(),
        ),
        (EDGE_AND_FACE, EDGE_AND_FACE),
        (ComponentMask::all(), ComponentMask::all()),
    ] {
        let mode = EditMode::from_components(components);

        assert_eq!(mode.renderable_components, renderable);
        assert_eq!(mode.selection_level, components);
    }
}
