//! Hidden components and the accessors used to read and edit them.

use event_emitter::EventEmitter;

use crate::{ComponentMask, ComponentSet, IntoComponentKey, MeshChange, Topology};

/// Options for [`Hidden::hide`] and [`Hidden::show`].
#[derive(Clone, Copy, Debug, Default)]
pub struct HideOptions {
    /// Also applies to faces using any of the given components, even if faces aren't
    /// interactable.
    pub extend_to_connected_faces: bool,
}

/// Editing accessor over a mesh's hidden components.
///
/// Components are hidden individually. Hiding or showing a component never affects other
/// components, unless requested with [`HideOptions`].
pub struct Hidden<'a> {
    pub(crate) hidden: &'a mut ComponentSet,
    pub(crate) topo: &'a Topology,
    pub(crate) interactable: ComponentMask,
    pub(crate) changes: &'a mut EventEmitter<MeshChange>,
}

impl Hidden<'_> {
    pub fn view(&self) -> &ComponentSet {
        todo!()
    }

    /// Hides the given components of interactable types.
    pub fn hide<K: IntoComponentKey>(&mut self, keys: &[K], options: HideOptions) {
        todo!()
    }

    /// Shows the given components of interactable types.
    pub fn show<K: IntoComponentKey>(&mut self, keys: &[K], options: HideOptions) {
        todo!()
    }

    /// Shows every hidden component, regardless of type.
    pub fn show_all(&mut self) {
        todo!()
    }
}

#[cfg(test)]
#[path = "../tests/unit/hidden.rs"]
mod tests;
