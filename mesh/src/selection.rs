//! Selection state and the accessors used to read and edit it.

mod boundary;
mod bounds;

pub use boundary::{SelectionBoundary, SelectionStep};

use std::collections::HashSet;
use std::ops::Deref;

use boundary::BoundaryCache;
use bounds::BoundsCache;
use event_emitter::EventEmitter;
use glam::Vec3;

use crate::{
    Aabb, AttributeError, AttributeKey, AttributeValue, Attributes, ComponentKey, ComponentKeys,
    ComponentMask, ComponentRef, ComponentSet, ComponentSetChange, ComponentType, IntoComponentKey,
    MeshChange, Topology,
};

#[derive(Default)]
pub(crate) struct SelectionState {
    pub(crate) components: ComponentSet,
    boundary: BoundaryCache,
    bounds: BoundsCache,
}

impl SelectionState {
    pub(crate) fn invalidate_boundary(&mut self) {
        self.boundary = BoundaryCache::default();
    }

    /// Call after selection edits and any position edit other than [`Self::translate_bounds`].
    pub(crate) fn invalidate_bounds(&mut self) {
        self.bounds = BoundsCache::default();
    }

    /// Keeps cached bounds exact after the selected vertices move by `delta`.
    pub(crate) fn translate_bounds(&mut self, delta: Vec3) {
        if let Some(Some(aabb)) = self.bounds.selected.get_mut() {
            *aabb = aabb.translated(delta);
        }
    }

    /// `None` for an empty selection.
    pub(crate) fn selected_bounds(&self, attrs: &Attributes) -> Option<Aabb> {
        *self.bounds.selected.get_or_init(|| {
            let positions = attrs.positions();
            Aabb::from_points(self.components.verts.iter().map(|&key| positions[key]))
        })
    }

    /// The bounds for the entire mesh -- combining the bounds of the selected components and unselected components. `None` for an empty mesh.
    pub(crate) fn mesh_bounds(&self, topo: &Topology, attrs: &Attributes) -> Option<Aabb> {
        let selected = self.selected_bounds(attrs);
        let unselected = *self.bounds.unselected.get_or_init(|| {
            let positions = attrs.positions();
            Aabb::from_points(
                topo.verts
                    .keys()
                    .filter(|key| !self.components.verts.contains(key))
                    .map(|key| positions[key]),
            )
        });

        match (selected, unselected) {
            (Some(selected), Some(unselected)) => Some(selected.union(unselected)),
            _ => selected.or(unselected),
        }
    }

    fn insert(&mut self, key: ComponentKey) -> bool {
        let changed = self.components.insert(key);
        if changed {
            self.invalidate_caches(key);
        }
        changed
    }

    fn remove(&mut self, key: ComponentKey) -> bool {
        let changed = self.components.remove(key);
        if changed {
            self.invalidate_caches(key);
        }
        changed
    }

    /// Invalidates caches affected by a membership change of `key`.
    fn invalidate_caches(&mut self, key: ComponentKey) {
        self.invalidate_boundary();
        if matches!(key, ComponentKey::Vert(_)) {
            self.invalidate_bounds();
        }
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
    pub(crate) interactable: ComponentMask,
    pub(crate) hidden: &'a ComponentSet,
    pub(crate) changes: &'a mut EventEmitter<MeshChange>,
}

/// Exposes the selected components.
impl Deref for SelectionView<'_> {
    type Target = ComponentSet;

    fn deref(&self) -> &ComponentSet {
        &self.state.components
    }
}

impl<'a> SelectionView<'a> {
    /// Lazy boundary sets shared across views, independent of the selection mode.
    pub fn boundary(&self) -> SelectionBoundary<'a> {
        SelectionBoundary::new(self.state, self.topo)
    }

    /// Local-space bounds of the selected vertices, or `None` for an empty selection.
    pub fn bounds(&self) -> Option<Aabb> {
        self.state.selected_bounds(self.attrs)
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

    /// Rebuilds the selection from selected components of interactable types.
    pub(crate) fn rebuild(&mut self) {
        let survivors: Vec<_> = self
            .view()
            .iter()
            .filter(|&key| self.interactable.contains(key.kind().into()))
            .collect();

        self.set(&survivors);
    }

    /// Sets the current selection to the given components, replacing any existing selection.
    pub fn set<K: IntoComponentKey>(&mut self, keys: &[K]) {
        let removed = self.clear_selection();

        let mut delta = self.apply_action(keys, Action::Select, self.interactable);
        delta.removed = removed;

        self.emit_change(delta);
    }

    /// Adds the given components to the current selection.
    pub fn select<K: IntoComponentKey>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Select, self.interactable);

        self.emit_change(delta);
    }

    /// Removes the given components from the current selection.
    pub fn deselect<K: IntoComponentKey>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Deselect, self.interactable);

        self.emit_change(delta);
    }

    /// Toggles the given components in the current selection.
    pub fn toggle<K: IntoComponentKey>(&mut self, keys: &[K]) {
        let delta = self.apply_action(keys, Action::Toggle, self.interactable);

        self.emit_change(delta);
    }

    /// Selects every component whose `name` attribute satisfies `predicate`, such as
    /// `|id: u32| id == 7`, skipping hidden components. Works just like [`Self::select`], so it
    /// does nothing unless `K`'s components are interactable.
    ///
    /// # Errors
    /// Like [`Attributes::values`].
    pub fn select_by_attribute<K: AttributeKey + IntoComponentKey, T: AttributeValue>(
        &mut self,
        name: &str,
        predicate: impl Fn(T) -> bool,
    ) -> Result<(), AttributeError> {
        let keys: Vec<K> = self
            .attrs
            .keys_where(name, predicate)?
            .filter(|&key| !self.hidden.contains(key))
            .collect();

        self.select(&keys);
        Ok(())
    }

    /// Clears all selected components, regardless of the current mode.
    pub fn clear(&mut self) {
        let delta = ComponentSetChange {
            removed: self.clear_selection(),
            ..ComponentSetChange::default()
        };

        self.emit_change(delta);
    }

    fn emit_change(&mut self, mut delta: ComponentSetChange) {
        delta.normalize();

        if !delta.is_empty() {
            self.changes.emit(MeshChange::Selection(delta));
        }
    }

    fn clear_selection(&mut self) -> ComponentKeys {
        if self.view().is_empty() {
            return ComponentKeys::default();
        }

        self.state.invalidate_boundary();
        self.state.invalidate_bounds();

        self.state.components.take()
    }

    /// Grow the selection by one outer layer of the current selection mode. Skips hidden components.
    /// Face-only mode edits faces; any vertex or edge mode edits vertices.
    /// A fully selected connected component has no boundary and is unchanged.
    pub fn grow(&mut self, step: SelectionStep) {
        let delta = if self.interactable == ComponentMask::FACE {
            let faces: Vec<_> = self
                .view()
                .boundary()
                .outer_faces(step)
                .filter(|&key| !self.hidden.contains(key))
                .collect();

            self.apply_action(&faces, Action::Select, ComponentMask::FACE)
        } else if self
            .interactable
            .intersects(ComponentMask::VERTEX | ComponentMask::EDGE)
        {
            let verts: Vec<_> = self
                .view()
                .boundary()
                .outer_vertices(step)
                .filter(|&key| !self.hidden.contains(key))
                .collect();

            self.apply_action(&verts, Action::Select, ComponentMask::VERTEX)
        } else {
            return;
        };

        self.emit_change(delta);
    }

    /// Shrink the selection by one inner layer, using the same mode rules as [`Self::grow`].
    pub fn shrink(&mut self, step: SelectionStep) {
        let delta = if self.interactable == ComponentMask::FACE {
            let faces: Vec<_> = self.view().boundary().inner_faces(step).collect();

            self.apply_action(&faces, Action::Deselect, ComponentMask::FACE)
        } else if self
            .interactable
            .intersects(ComponentMask::VERTEX | ComponentMask::EDGE)
        {
            let verts: Vec<_> = self.view().boundary().inner_vertices(step).collect();

            self.apply_action(&verts, Action::Deselect, ComponentMask::VERTEX)
        } else {
            return;
        };

        self.emit_change(delta);
    }

    /// Takes a set of components by key and applies a selection action to them (select, remove, toggle).
    /// Selection actions apply not only to directly-selected components, but also propagate to sub- or super-components.
    fn apply_action<K: IntoComponentKey>(
        &mut self,
        keys: &[K],
        action: Action,
        allowed_kinds: ComponentMask,
    ) -> ComponentSetChange {
        let mut delta = ComponentSetChange::default();
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
                delta.added.push(key);
            } else if !should_select && self.state.remove(key) {
                delta.removed.push(key);
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
    fn propagate_removals(&mut self, removed: &mut ComponentKeys) {
        let mut upward_sources = removed.clone();
        let mut possible_orphans = HashSet::new();
        let mut lower_components = Vec::new();
        let mut upper_components = Vec::new();

        // Propagate removals downward through the component hierarchy
        for (_, upper_kind) in Self::COMPONENT_KIND_PAIRS.into_iter().rev() {
            possible_orphans.clear();
            for key in removed.iter(upper_kind) {
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
                    removed.push(key);
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
    fn propagate_additions(&mut self, added: &mut ComponentKeys) {
        let mut upward_sources = added.clone();
        let mut lower_components = Vec::new();

        // Propagate additions downward through the component hierarchy
        for (_, upper_kind) in Self::COMPONENT_KIND_PAIRS.into_iter().rev() {
            for index in 0..added.len(upper_kind) {
                // Get the lower components of the current upper component (e.g. the vertices of an added edge)
                let upper_key = added.get(upper_kind, index).unwrap();
                ComponentRef::new(upper_key, self.topo, self.attrs).lower(&mut lower_components);

                for &key in &lower_components {
                    if self.state.insert(key) {
                        added.push(key);
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
        selection_changes: &mut ComponentKeys,
        upward_sources: &mut ComponentKeys,
    ) {
        let should_select = matches!(action, PropagationAction::Select);
        let mut upper_components_to_check = HashSet::new();
        let mut lower_components = Vec::new();
        let mut upper_components = Vec::new();

        for (lower_kind, _) in Self::COMPONENT_KIND_PAIRS {
            upper_components_to_check.clear();
            for key in upward_sources.iter(lower_kind) {
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
                    selection_changes.push(key);
                    upward_sources.push(key);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/selection.rs"]
mod tests;
