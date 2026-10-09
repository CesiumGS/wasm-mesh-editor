//! Selection state and the accessors used to read and edit it.

mod boundary;

pub use boundary::{SelectionBoundary, SelectionStep};

use std::collections::HashSet;
use std::rc::Rc;

use boundary::BoundaryCache;
use event_emitter::EventEmitter;

use crate::mesh_component::PerComponentType;
use crate::{
    Attributes, ComponentKey, ComponentRef, ComponentType, ComponentTypes, EdgeKey, FaceKey,
    MeshChange, SelectionChange, Topology, VertKey,
};

/// A component kind that can be selected: the three handle types and
/// [`ComponentKey`]. A named alias over `Copy + Into<ComponentKey>`.
pub trait SelectionKind: Copy + Into<ComponentKey> {}

impl<T: Copy + Into<ComponentKey>> SelectionKind for T {}

pub(crate) struct SelectionState {
    pub(crate) verts: Rc<HashSet<VertKey>>,
    pub(crate) edges: HashSet<EdgeKey>,
    pub(crate) faces: HashSet<FaceKey>,
    pub(crate) level: ComponentTypes,
    boundary: BoundaryCache,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            verts: Rc::default(),
            edges: HashSet::new(),
            faces: HashSet::new(),
            level: ComponentTypes::VERTEX,
            boundary: BoundaryCache::default(),
        }
    }
}

impl SelectionState {
    pub(crate) fn invalidate_boundary(&mut self) {
        self.boundary = BoundaryCache::default();
    }

    fn insert(&mut self, key: ComponentKey) -> bool {
        let changed = match key {
            ComponentKey::Vert(key) => {
                if self.verts.contains(&key) {
                    return false;
                }

                Rc::make_mut(&mut self.verts).insert(key)
            }
            ComponentKey::Edge(key) => self.edges.insert(key),
            ComponentKey::Face(key) => self.faces.insert(key),
        };
        if changed {
            self.invalidate_boundary();
        }
        changed
    }

    fn remove(&mut self, key: ComponentKey) -> bool {
        let changed = match key {
            ComponentKey::Vert(key) => {
                if !self.verts.contains(&key) {
                    return false;
                }

                Rc::make_mut(&mut self.verts).remove(&key)
            }
            ComponentKey::Edge(key) => self.edges.remove(&key),
            ComponentKey::Face(key) => self.faces.remove(&key),
        };
        if changed {
            self.invalidate_boundary();
        }
        changed
    }
}

enum Action {
    Select,
    Deselect,
    Toggle,
}

enum PropagationAction {
    Select,
    Deselect,
}

/// Read-only accessor over a mesh's selection.
pub struct SelectionView<'a> {
    pub(crate) state: &'a SelectionState,
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
}

/// Editing accessor over a mesh's selection.
///
/// Selected faces pull in their edges, and selected edges pull in their vertices.
/// Only direct additions (and promotions) trigger upward selection of components
/// whose lower components are all selected. Cascaded additions do not.
/// Removals first discard orphaned lower components, then incomplete upper ones.
pub struct Selection<'a> {
    pub(crate) state: &'a mut SelectionState,
    pub(crate) topo: &'a Topology,
    pub(crate) attrs: &'a Attributes,
    pub(crate) changes: &'a mut EventEmitter<MeshChange>,
}

impl<'a> SelectionView<'a> {
    /// The component kinds that accept direct selection input.
    pub fn level(&self) -> ComponentTypes {
        self.state.level
    }

    pub fn verts(&self) -> impl Iterator<Item = VertKey> + '_ {
        self.state.verts.iter().copied()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeKey> + '_ {
        self.state.edges.iter().copied()
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        self.state.faces.iter().copied()
    }

    /// Every selected component as a tagged handle, in unspecified order within each kind.
    pub fn selected(&self) -> impl Iterator<Item = ComponentKey> + '_ {
        self.verts()
            .map(ComponentKey::Vert)
            .chain(self.edges().map(ComponentKey::Edge))
            .chain(self.faces().map(ComponentKey::Face))
    }

    /// The total number of selected components across all kinds.
    pub fn len(&self) -> usize {
        self.state.verts.len() + self.state.edges.len() + self.state.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains<K: SelectionKind>(&self, key: K) -> bool {
        match key.into() {
            ComponentKey::Vert(key) => self.state.verts.contains(&key),
            ComponentKey::Edge(key) => self.state.edges.contains(&key),
            ComponentKey::Face(key) => self.state.faces.contains(&key),
        }
    }

    /// Lazy boundary sets shared across views, independent of the selection mode.
    pub fn boundary(&self) -> SelectionBoundary<'a> {
        SelectionBoundary::new(self.state, self.topo)
    }
}

impl<'a> Selection<'a> {
    const COMPONENT_KIND_PAIRS: [(ComponentType, ComponentType); 2] = [
        (ComponentType::Vertex, ComponentType::Edge),
        (ComponentType::Edge, ComponentType::Face),
    ];

    /// A read-only view of this selection.
    pub fn view(&self) -> SelectionView<'_> {
        SelectionView {
            state: self.state,
            topo: self.topo,
            attrs: self.attrs,
        }
    }

    /// Sets the component kinds that accept direct selection input.
    /// Combine kinds with `|`, such as `ComponentTypes::VERTEX | ComponentTypes::EDGE`.
    /// Other kinds may still be selected indirectly by downward or upward propagation.
    /// Changing the mode rebuilds the selection from selected components in the new mode.
    pub fn set_level(&mut self, level: ComponentTypes) {
        if self.state.level == level {
            return;
        }

        let survivors: Vec<_> = self
            .view()
            .selected()
            .filter(|&key| level.contains(key.kind().into()))
            .collect();

        self.state.level = level;

        self.set(&survivors);
    }

    /// Sets the current selection to the given components, replacing any existing selection.
    pub fn set<K: SelectionKind>(&mut self, keys: &[K]) {
        let removed = self.clear_selection();

        let mut delta = self.apply_action(keys, Action::Select, self.state.level);
        delta.removed = removed;

        self.emit_change(delta);
    }

    /// Adds the given components to the current selection.
    pub fn select<K: SelectionKind>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Select, self.state.level);

        self.emit_change(delta);
    }

    /// Removes the given components from the current selection.
    pub fn deselect<K: SelectionKind>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Deselect, self.state.level);

        self.emit_change(delta);
    }

    /// Toggles the given components in the current selection.
    pub fn toggle<K: SelectionKind>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Toggle, self.state.level);

        self.emit_change(delta);
    }

    /// Clears all selected components, regardless of the current mode.
    pub fn clear(&mut self) {
        let delta = SelectionChange {
            removed: self.clear_selection(),
            ..SelectionChange::default()
        };

        self.emit_change(delta);
    }

    fn emit_change(&mut self, mut delta: SelectionChange) {
        delta.normalize();

        if !delta.is_empty() {
            self.changes.emit(MeshChange::Selection(delta));
        }
    }

    fn clear_selection(&mut self) -> PerComponentType<Vec<ComponentKey>> {
        if self.view().is_empty() {
            return PerComponentType::default();
        }

        self.state.invalidate_boundary();

        let mut removed = PerComponentType::default();

        removed[ComponentType::Vertex] = match Rc::get_mut(&mut self.state.verts) {
            Some(verts) => verts.drain().map(ComponentKey::Vert).collect(),
            None => {
                let verts = std::mem::take(&mut self.state.verts);

                verts.iter().copied().map(ComponentKey::Vert).collect()
            }
        };
        removed[ComponentType::Edge] = self.state.edges.drain().map(ComponentKey::Edge).collect();
        removed[ComponentType::Face] = self.state.faces.drain().map(ComponentKey::Face).collect();

        removed
    }

    /// Grow the selection by one outer layer of the current selection mode.
    /// Face-only mode edits faces; any vertex or edge mode edits vertices.
    /// A fully selected connected component has no boundary and is unchanged.
    pub fn grow(&mut self, step: SelectionStep) {
        let delta = if self.state.level == ComponentTypes::FACE {
            let faces: Vec<_> = self.view().boundary().outer_faces(step).collect();

            self.apply_action(&faces, Action::Select, ComponentTypes::FACE)
        } else if self
            .state
            .level
            .intersects(ComponentTypes::VERTEX | ComponentTypes::EDGE)
        {
            let verts: Vec<_> = self.view().boundary().outer_vertices(step).collect();

            self.apply_action(&verts, Action::Select, ComponentTypes::VERTEX)
        } else {
            return;
        };

        self.emit_change(delta);
    }

    /// Shrink the selection by one inner layer, using the same mode rules as [`Self::grow`].
    pub fn shrink(&mut self, step: SelectionStep) {
        let delta = if self.state.level == ComponentTypes::FACE {
            let faces: Vec<_> = self.view().boundary().inner_faces(step).collect();

            self.apply_action(&faces, Action::Deselect, ComponentTypes::FACE)
        } else if self
            .state
            .level
            .intersects(ComponentTypes::VERTEX | ComponentTypes::EDGE)
        {
            let verts: Vec<_> = self.view().boundary().inner_vertices(step).collect();

            self.apply_action(&verts, Action::Deselect, ComponentTypes::VERTEX)
        } else {
            return;
        };

        self.emit_change(delta);
    }

    /// Takes a set of components by key and applies a selection action to them (select, remove, toggle).
    /// Selection actions apply not only to directly-selected components, but also propagate to sub- or super-components.
    fn apply_action<K: SelectionKind>(
        &mut self,
        keys: &[K],
        action: Action,
        allowed_kinds: ComponentTypes,
    ) -> SelectionChange {
        let mut delta = SelectionChange::default();
        let mut seen_keys = HashSet::new();

        for &key in keys {
            let key = key.into();

            if !seen_keys.insert(key)
                || !allowed_kinds.contains(key.kind().into())
                || !self.topo.contains(key)
            {
                continue;
            }

            let should_select = match action {
                Action::Select => true,
                Action::Deselect => false,
                Action::Toggle => !self.view().contains(key),
            };

            if should_select && self.state.insert(key) {
                delta.added[key.kind()].push(key);
            } else if !should_select && self.state.remove(key) {
                delta.removed[key.kind()].push(key);
            }
        }

        self.propagate_removals(&mut delta.removed);
        self.propagate_additions(&mut delta.added);

        delta
    }

    /// Propagates deselection of directly-deselected components to their sub- and
    /// super-components.
    ///
    /// Example 1: if an edge is deselected, each of its vertices is removed from
    /// the selection unless it belongs to another selected edge. Any selected
    /// faces containing that edge are deselected, too, since they no longer have
    /// all of their edges selected.
    ///
    /// Importantly, deselection changes that propagate upwards don't then propagate
    /// back down to deselect other sub-components...
    ///
    /// Example 2: deselecting one vertex of a fully selected triangle deselects
    /// its two incident edges and the face. However, the other two vertices and the
    /// opposite edge remain selected.
    fn propagate_removals(&mut self, removed: &mut PerComponentType<Vec<ComponentKey>>) {
        let mut upward_sources = removed.clone();
        let mut possible_orphans = HashSet::new();
        let mut lower_components = Vec::new();
        let mut upper_components = Vec::new();

        // Propagate removals downward through the component hierarchy
        for (lower_kind, upper_kind) in Self::COMPONENT_KIND_PAIRS.into_iter().rev() {
            possible_orphans.clear();
            for &key in &removed[upper_kind] {
                ComponentRef::new(key, self.topo, self.attrs).lower(&mut lower_components);
                possible_orphans.extend(
                    lower_components
                        .iter()
                        .copied()
                        .filter(|&key| self.view().contains(key)),
                );
            }
            for &key in &possible_orphans {
                ComponentRef::new(key, self.topo, self.attrs).upper(&mut upper_components);
                if !upper_components
                    .iter()
                    .any(|&key| self.view().contains(key))
                    && self.state.remove(key)
                {
                    removed[lower_kind].push(key);
                }
            }
        }

        self.propagate_upward(PropagationAction::Deselect, removed, &mut upward_sources);
    }

    /// Propagates selection of directly-selected components to their sub- and super-components.
    ///
    /// Example 1: if an edge is selected, both of its vertices are added to the selection as well. If the edge
    /// is the final edge to be selected in any face that edge participates in, those face(s) will be added to the selection, too.
    ///
    /// Importantly, selection changes that propagate downwards don't then participate in upward propagation. Only the initial selection does that...
    ///
    /// Example 2: selecting three edges of a quad propagates down to select all four vertices of the quad. However, the face itself will remain unselected.
    fn propagate_additions(&mut self, added: &mut PerComponentType<Vec<ComponentKey>>) {
        let mut upward_sources = added.clone();
        let mut lower_components = Vec::new();

        // Propagate additions downward through the component hierarchy
        for (lower_kind, upper_kind) in Self::COMPONENT_KIND_PAIRS.into_iter().rev() {
            for index in 0..added[upper_kind].len() {
                // Get the lower components of the current upper component (e.g. the vertices of an added edge)
                let upper_key = added[upper_kind][index];
                ComponentRef::new(upper_key, self.topo, self.attrs).lower(&mut lower_components);

                for &key in &lower_components {
                    if self.state.insert(key) {
                        added[lower_kind].push(key);
                    }
                }
            }
        }

        self.propagate_upward(PropagationAction::Select, added, &mut upward_sources);
    }

    /// Propagates selection changes upward through the component hierarchy.
    /// NOTE: `selection_changes` are passed separately from `upward_sources`, even though `upward_sources` is a snapshot of `selection_changes` from before the downward propagation.
    /// This ensures that we only propagate changes upward that are actually caused by the _initial_ selection action.
    /// For instance: if we select two edges of a triangle, downward propagation adds all 3 vertices of those edges to the selection. The subsequent upward propagation
    /// should NOT select the third edge of the triangle, nor the triangle itself. Thus, it must be based on the propagation sources rather than the selection changes, which include changes by downward propagation.
    fn propagate_upward(
        &mut self,
        action: PropagationAction,
        selection_changes: &mut PerComponentType<Vec<ComponentKey>>,
        upward_sources: &mut PerComponentType<Vec<ComponentKey>>,
    ) {
        let should_select = matches!(action, PropagationAction::Select);
        let mut upper_components_to_check = HashSet::new();
        let mut lower_components = Vec::new();
        let mut upper_components = Vec::new();

        for (lower_kind, upper_kind) in Self::COMPONENT_KIND_PAIRS {
            upper_components_to_check.clear();
            for &key in &upward_sources[lower_kind] {
                ComponentRef::new(key, self.topo, self.attrs).upper(&mut upper_components);

                upper_components_to_check.extend(
                    upper_components
                        .iter()
                        .copied()
                        .filter(|&key| self.view().contains(key) != should_select),
                );
            }

            for &key in &upper_components_to_check {
                ComponentRef::new(key, self.topo, self.attrs).lower(&mut lower_components);

                let all_lower_selected = lower_components
                    .iter()
                    .all(|&key| self.view().contains(key));

                if all_lower_selected != should_select {
                    continue;
                }

                let changed = if should_select {
                    self.state.insert(key)
                } else {
                    self.state.remove(key)
                };

                if changed {
                    selection_changes[upper_kind].push(key);
                    upward_sources[upper_kind].push(key);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/selection.rs"]
mod tests;
