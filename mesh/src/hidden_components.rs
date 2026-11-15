//! Hidden components and the accessors used to read and edit them.

use event_emitter::EventEmitter;

use crate::{
    ComponentKey, ComponentMask, ComponentSet, ComponentSetChange, FaceKey, IntoComponentKey,
    MeshChange, Topology,
};

/// Options for [`HiddenComponents::hide`] and [`HiddenComponents::show`].
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
pub struct HiddenComponents<'a> {
    pub(crate) hidden: &'a mut ComponentSet,
    pub(crate) topo: &'a Topology,
    pub(crate) interactable: ComponentMask,
    pub(crate) changes: &'a mut EventEmitter<MeshChange>,
}

impl HiddenComponents<'_> {
    pub fn view(&self) -> &ComponentSet {
        self.hidden
    }

    /// Hides the given components of interactable types.
    pub fn hide<K: IntoComponentKey>(&mut self, keys: &[K], options: HideOptions) {
        self.apply(keys, options, true);
    }

    /// Shows the given components of interactable types.
    pub fn show<K: IntoComponentKey>(&mut self, keys: &[K], options: HideOptions) {
        self.apply(keys, options, false);
    }

    /// Shows every hidden component, regardless of type.
    pub fn show_all(&mut self) {
        let change = ComponentSetChange {
            removed: self.hidden.take(),
            ..ComponentSetChange::default()
        };

        self.emit_change(change);
    }

    fn apply<K: IntoComponentKey>(&mut self, keys: &[K], options: HideOptions, hide: bool) {
        let mut keys = self.filter_by_interactable(keys);

        if options.extend_to_connected_faces {
            let faces = self.connected_faces(&keys);
            keys.extend(faces.into_iter().map(ComponentKey::Face));
        }

        let update: fn(&mut ComponentSet, ComponentKey) -> bool = if hide {
            ComponentSet::insert
        } else {
            ComponentSet::remove
        };

        let mut change = ComponentSetChange::default();
        let changed = if hide {
            &mut change.added
        } else {
            &mut change.removed
        };

        for key in keys {
            if update(self.hidden, key) {
                changed.push(key);
            }
        }

        self.emit_change(change);
    }

    /// The given keys that exist and satisfy the interactable component mask.
    fn filter_by_interactable<K: IntoComponentKey>(&self, keys: &[K]) -> Vec<ComponentKey> {
        keys.iter()
            .map(|&key| key.into())
            .filter(|&key| self.interactable.contains(key.kind().into()) && self.topo.contains(key))
            .collect()
    }

    /// Faces using any of the given vertices or edges, possibly with duplicates.
    fn connected_faces(&self, keys: &[ComponentKey]) -> Vec<FaceKey> {
        let mut faces = Vec::new();

        for &key in keys {
            match key {
                ComponentKey::Vert(key) => faces.extend(self.topo.vert_faces(key)),
                ComponentKey::Edge(key) => faces.extend(self.topo.edge_faces(key)),
                ComponentKey::Face(_) => {}
            }
        }

        faces
    }

    fn emit_change(&mut self, change: ComponentSetChange) {
        if !change.is_empty() {
            self.changes.emit(MeshChange::Hidden(change));
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/hidden_components.rs"]
mod tests;
