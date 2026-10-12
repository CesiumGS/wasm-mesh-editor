use mesh_core::ComponentMask;

/// Which components are visible and directly selectable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditMode {
    pub renderable_components: ComponentMask,
    pub selection_level: ComponentMask,
}

impl EditMode {
    pub const NONE: Self = Self {
        renderable_components: ComponentMask::empty(),
        selection_level: ComponentMask::empty(),
    };

    pub const VERTEX: Self = Self {
        renderable_components: ComponentMask::all(),
        selection_level: ComponentMask::VERTEX,
    };

    pub const EDGE: Self = Self {
        renderable_components: ComponentMask::EDGE.union(ComponentMask::FACE),
        selection_level: ComponentMask::EDGE,
    };

    pub const FACE: Self = Self {
        renderable_components: ComponentMask::EDGE.union(ComponentMask::FACE),
        selection_level: ComponentMask::FACE,
    };
}
