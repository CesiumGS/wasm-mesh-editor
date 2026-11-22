use mesh::ComponentMask;

/// Which components are visible and interactable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditMode {
    /// Component visibility policy for the renderer.
    pub renderable_components: ComponentMask,
    pub interactable_components: ComponentMask,
}

impl EditMode {
    /// Nothing is interactable; faces are still drawn so the mesh stays visible.
    pub const NONE: Self = Self::from_components(ComponentMask::empty());

    pub const VERTEX: Self = Self::from_components(ComponentMask::VERTEX);

    pub const EDGE: Self = Self::from_components(ComponentMask::EDGE);

    pub const FACE: Self = Self::from_components(ComponentMask::FACE);

    /// The mode in which `components` are interactable. Each mode comes
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
            interactable_components: components,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/edit_mode.rs"]
mod tests;
