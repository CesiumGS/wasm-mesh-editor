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
    /// Nothing is selectable; faces are still drawn so the mesh stays visible.
    pub const NONE: Self = Self::from_components(ComponentMask::empty());

    pub const VERTEX: Self = Self::from_components(ComponentMask::VERTEX);

    pub const EDGE: Self = Self::from_components(ComponentMask::EDGE);

    pub const FACE: Self = Self::from_components(ComponentMask::FACE);

    /// The mode in which `components` are selectable. Each mode comes
    /// with a policy for which components should be rendered.
    pub const fn from_components(components: ComponentMask) -> Self {
        // Always render at least faces (even in NONE mode).
        let mut renderable_components = ComponentMask::FACE;

        // Edges are often rendered even outside of dedicated edge mode, for context and visibility.
        if !components.is_empty() {
            renderable_components = renderable_components.union(ComponentMask::EDGE);
        }

        if components.contains(ComponentMask::VERTEX) {
            renderable_components = renderable_components.union(ComponentMask::VERTEX);
        }

        Self {
            renderable_components,
            selection_level: components,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/edit_mode.rs"]
mod tests;
