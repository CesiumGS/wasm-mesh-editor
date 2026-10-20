use mesh::ComponentMask;

/// Which components are visible and directly selectable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub struct EditMode {
    /// Component visibility policy for the renderer.
    #[cfg_attr(feature = "wasm", wasm_bindgen(skip))]
    pub renderable_components: ComponentMask,
    #[cfg_attr(feature = "wasm", wasm_bindgen(skip))]
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
