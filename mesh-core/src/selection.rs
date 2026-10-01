//! Selection state and the accessors used to read and edit it.

mod boundary;

pub use boundary::SelectionBoundary;

use std::collections::HashSet;

use crate::{Attributes, ComponentKey, ComponentTypes, EdgeKey, FaceKey, Topology, VertKey};

/// A component kind that can be selected: the three handle types and
/// [`ComponentKey`]. A named alias over `Copy + Into<ComponentKey>`.
pub trait SelectionKind: Copy + Into<ComponentKey> {}

impl<T: Copy + Into<ComponentKey>> SelectionKind for T {}

pub(crate) struct SelectionState {
    pub(crate) verts: HashSet<VertKey>,
    pub(crate) edges: HashSet<EdgeKey>,
    pub(crate) faces: HashSet<FaceKey>,
    pub(crate) level: ComponentTypes,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            verts: HashSet::new(),
            edges: HashSet::new(),
            faces: HashSet::new(),
            level: ComponentTypes::VERTEX,
        }
    }
}

/// Read-only accessor over a mesh's selection.
pub struct SelectionView<'a> {
    pub(crate) state: &'a SelectionState,
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
}

/// Editing accessor over a mesh's selection. Applies down-cascade and up-bubble
/// on every change.
pub struct Selection<'a> {
    pub(crate) state: &'a mut SelectionState,
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
}

impl<'a> SelectionView<'a> {
    /// The component kinds that accept direct selection input.
    pub fn level(&self) -> ComponentTypes {
        todo!()
    }

    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        std::iter::empty::<EdgeKey>()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }

    /// Every selected component as a tagged handle (a flat snapshot).
    pub fn selected(&self) -> impl Iterator<Item = ComponentKey> + '_ {
        std::iter::empty::<ComponentKey>()
    }

    pub fn contains<K: SelectionKind>(&self, key: K) -> bool {
        todo!()
    }

    pub fn boundary(&self) -> SelectionBoundary {
        todo!()
    }
}

impl<'a> Selection<'a> {
    /// A read-only view of this selection.
    pub fn view(&self) -> SelectionView<'_> {
        todo!()
    }

    /// Sets the component kinds that accept direct selection input.
    /// Combine kinds with `|`, such as `ComponentTypes::VERTEX | ComponentTypes::EDGE`.
    /// Other kinds may still be selected indirectly by down-cascade or up-bubble.
    pub fn set_level(&mut self, level: ComponentTypes) {
        todo!()
    }

    /// Sets the current selection to the given components, replacing any existing selection.
    pub fn set<K: SelectionKind>(&mut self, keys: &[K]) {
        todo!()
    }

    /// Adds the given components to the current selection.
    pub fn select<K: SelectionKind>(&mut self, keys: &[K]) {
        todo!()
    }

    /// Removes the given components from the current selection.
    pub fn deselect<K: SelectionKind>(&mut self, keys: &[K]) {
        todo!()
    }

    /// Toggles the given components in the current selection.
    pub fn toggle<K: SelectionKind>(&mut self, keys: &[K]) {
        todo!()
    }

    pub fn clear(&mut self) {
        todo!()
    }

    pub fn grow(&mut self) {
        todo!()
    }

    pub fn shrink(&mut self) {
        todo!()
    }
}
