//! Hidden components and the accessors used to read and edit them.

use event_emitter::EventEmitter;

use crate::{
    AttributeError, AttributeKey, AttributeValue, Attributes, ComponentKey, ComponentMask,
    ComponentSet, ComponentSetChange, FaceKey, IntoComponentKey, MeshChange, Topology,
};

/// Options for [`HiddenComponents::hide`] and [`HiddenComponents::show`].
#[derive(Clone, Copy, Debug, Default)]
pub struct HideOptions {
    /// Also applies to faces using any of the given components, even if faces aren't in the interactable component mask.
    pub extend_to_connected_faces: bool,
}

/// The hidden components of a mesh, and the functions for hiding and showing them.
///
/// A hidden component is one the user isn't meant to interact with. Whether a hidden mesh component is actually drawn is
/// a renderer concern. Regardless, hidden components still have meaning in the context of mesh operations.
/// Selection operations that do not directly specify hidden components, skip them (e.g. [`Selection::grow`]).
/// The intent is to prevent unexpected interactions with hidden components -- e.g. translation of hidden components that
/// might have otherwise been included by a grow operation.
///
/// Hiding one component doesn't hide related components, unless
/// [`HideOptions::extend_to_connected_faces`] is set.
pub struct HiddenComponents<'a> {
    pub(crate) hidden: &'a mut ComponentSet,
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
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

    /// Hides every component whose `name` attribute satisfies `predicate`. Works just like
    /// [`Self::hide`], so it does nothing unless `K`'s components are interactable.
    ///
    /// # Errors
    /// Like [`Attributes::values`].
    pub fn hide_by_attribute<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &mut self,
        name: &str,
        predicate: impl Fn(T) -> bool,
        options: HideOptions,
    ) -> Result<(), AttributeError> {
        let keys: Vec<K> = self.attrs.keys_where(name, predicate)?.collect();

        self.hide(&keys, options);
        Ok(())
    }

    /// Shows every component whose `name` attribute satisfies `predicate`, like
    /// [`Self::hide_by_attribute`].
    ///
    /// # Errors
    /// Like [`Attributes::values`].
    pub fn show_by_attribute<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &mut self,
        name: &str,
        predicate: impl Fn(T) -> bool,
        options: HideOptions,
    ) -> Result<(), AttributeError> {
        let keys: Vec<K> = self.attrs.keys_where(name, predicate)?.collect();

        self.show(&keys, options);
        Ok(())
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
