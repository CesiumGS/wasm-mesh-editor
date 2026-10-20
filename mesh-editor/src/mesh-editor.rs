use std::collections::HashMap;

use mesh::Mesh;
use slotmap::{SecondaryMap, SlotMap, new_key_type};
use topology_overlay::{OverlayBuffers, TopologyOverlay};

use crate::{EditMode, Tool, ToolAccessor, ToolContext, ToolHandle, ToolId};

#[path = "mesh-editor/errors.rs"]
mod errors;
#[path = "mesh-editor/update_result.rs"]
mod update_result;

pub use errors::{MeshNotFound, ToolActivationError};
pub use update_result::UpdateResult;

new_key_type! {
    /// Identifies a mesh registered with an editor.
    pub struct MeshKey;
}

/// Owns registered meshes and coordinates their overlays and active tool.
/// Tool, mode, and active-mesh changes cancel the current interaction first.
pub struct MeshEditor {
    meshes: SlotMap<MeshKey, Mesh>,
    overlays: SecondaryMap<MeshKey, TopologyOverlay>,
    mesh_keys: Vec<MeshKey>,
    active_meshes: Vec<MeshKey>,
    mode: EditMode,
    tools: HashMap<ToolId, Box<dyn Tool>>,
    active_tool: Option<ToolId>,
}

impl Default for MeshEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshEditor {
    /// Creates an editor with no meshes or active tool, in `EditMode::NONE`.
    pub fn new() -> Self {
        Self {
            meshes: SlotMap::with_key(),
            overlays: SecondaryMap::new(),
            mesh_keys: Vec::new(),
            active_meshes: Vec::new(),
            mode: EditMode::NONE,
            tools: HashMap::new(),
            active_tool: None,
        }
    }

    /// Returns the editing policy, including component visibility for the renderer.
    pub fn mode(&self) -> EditMode {
        self.mode
    }

    /// Registers a mesh and builds its topology overlay. The mesh starts inactive.
    pub fn add_mesh(&mut self, mut mesh: Mesh) -> MeshKey {
        let overlay = TopologyOverlay::new(&mut mesh);
        let key = self.meshes.insert(mesh);

        self.overlays.insert(key, overlay);
        self.mesh_keys.push(key);

        key
    }

    /// Removes a registered mesh from the editor and returns ownership to the caller.
    pub fn remove_mesh(&mut self, key: MeshKey) -> Option<Mesh> {
        self.meshes.get(key)?;

        if self.is_mesh_active(key) {
            self.cancel_active_tool();
        }

        let mesh = self.meshes.remove(key)?;

        self.overlays.remove(key);
        self.mesh_keys.retain(|&mesh_key| mesh_key != key);
        self.active_meshes.retain(|&mesh_key| mesh_key != key);

        Some(mesh)
    }

    pub fn has_mesh(&self, key: MeshKey) -> bool {
        self.meshes.contains_key(key)
    }

    pub fn is_mesh_active(&self, key: MeshKey) -> bool {
        self.active_meshes.contains(&key)
    }

    /// Applies the current mode to this mesh. Returns [`MeshNotFound`] for an unknown key.
    pub fn set_mesh_active(&mut self, key: MeshKey) -> Result<(), MeshNotFound> {
        self.meshes.get(key).ok_or(MeshNotFound { key })?;

        if self.active_meshes.contains(&key) {
            return Ok(());
        }

        self.cancel_active_tool();

        self.meshes[key]
            .selection_mut()
            .set_level(self.mode.selection_level);
        self.active_meshes.push(key);

        Ok(())
    }

    /// Stops editing this mesh without changing its selection or selection level.
    /// Returns [`MeshNotFound`] for an unknown key.
    pub fn set_mesh_inactive(&mut self, key: MeshKey) -> Result<(), MeshNotFound> {
        self.meshes.get(key).ok_or(MeshNotFound { key })?;

        if !self.active_meshes.contains(&key) {
            return Ok(());
        }

        self.cancel_active_tool();

        self.active_meshes.retain(|&mesh_key| mesh_key != key);

        Ok(())
    }

    /// Advances the active tool, then updates all registered overlays.
    /// Called by the consumer's render loop.
    /// Clears and refills `result`, reusing its capacity.
    pub fn update(&mut self, delta_seconds: f64, result: &mut UpdateResult) {
        result.changed_overlays.clear();

        if let Some(mut tool_accessor) = self.active_tool_mut() {
            tool_accessor
                .tool
                .update(&mut tool_accessor.context, delta_seconds);
        }

        for (key, overlay) in &mut self.overlays {
            if overlay.update(&self.meshes[key]).is_some() {
                result.changed_overlays.push(key);
            }
        }
    }
}

impl MeshEditor {
    /// Applies the mode's selection level to active meshes and deactivates incompatible tools.
    pub fn set_mode(&mut self, mode: EditMode) {
        if self.mode == mode {
            return;
        }

        self.update_active_tool_mode(mode);
        self.mode = mode;

        for &key in &self.active_meshes {
            self.meshes[key]
                .selection_mut()
                .set_level(mode.selection_level);
        }
    }

    /// Returns a read-only view of a registered mesh.
    pub fn mesh(&self, key: MeshKey) -> Option<&Mesh> {
        self.meshes.get(key)
    }

    pub fn mesh_keys(&self) -> &[MeshKey] {
        &self.mesh_keys
    }

    pub fn active_meshes(&self) -> &[MeshKey] {
        &self.active_meshes
    }

    /// Registers a tool with this editor. The tool starts inactive.
    pub fn add_tool<ToolType: Tool>(&mut self, tool: ToolType) -> ToolHandle<ToolType> {
        ToolHandle::new(self.add_boxed_tool(Box::new(tool)))
    }

    /// Registers a type-erased tool, for callers without a concrete tool type (such as JS).
    pub(crate) fn add_boxed_tool(&mut self, tool: Box<dyn Tool>) -> ToolId {
        let id = ToolId::new();

        self.tools.insert(id, tool);

        id
    }

    pub fn active_tool(&self) -> Option<ToolId> {
        self.active_tool
    }

    /// Borrows the active tool and constructs its mesh context.
    pub fn active_tool_mut(&mut self) -> Option<ToolAccessor<'_>> {
        let id = self.active_tool?;

        self.tool_mut(id)
    }

    /// Borrows a tool only if its ID belongs to this editor and it is active.
    /// Retained handles use this to avoid dispatching to a different tool.
    pub fn tool_mut(&mut self, id: ToolId) -> Option<ToolAccessor<'_>> {
        if self.active_tool != Some(id) {
            return None;
        }

        let tool = self.tools.get_mut(&id)?.as_mut();
        let context = ToolContext::new(&mut self.meshes, &self.active_meshes, self.mode);

        Some(ToolAccessor {
            tool,
            context,
            overlays: &self.overlays,
        })
    }

    /// Borrows a concrete tool for configuration, active or inactive.
    /// Returns `None` for a foreign or missing handle.
    ///
    /// ```no_run
    /// use mesh_editor::{MeshEditor, Tool};
    ///
    /// fn register_and_configure<ToolType: Tool>(
    ///     editor: &mut MeshEditor,
    ///     tool: ToolType,
    /// ) -> &mut ToolType {
    ///     let handle = editor.add_tool(tool);
    ///     editor.set_active_tool(Some(handle.id())).unwrap();
    ///
    ///     editor.tool_config_mut(&handle).unwrap()
    /// }
    /// ```
    pub fn tool_config_mut<ToolType: Tool>(
        &mut self,
        handle: &ToolHandle<ToolType>,
    ) -> Option<&mut ToolType> {
        self.tools
            .get_mut(&handle.id())?
            .as_any_mut()
            .downcast_mut::<ToolType>()
    }

    /// Selects a registered tool, or clears the active tool with `None`.
    /// Invalid requests leave the current tool unchanged. Repeated selections do nothing.
    pub fn set_active_tool(&mut self, tool: Option<ToolId>) -> Result<(), ToolActivationError> {
        if self.active_tool == tool {
            return Ok(());
        }

        self.validate_tool_activation(tool)?;

        if let Some(mut active) = self.active_tool_mut() {
            active.cancel();
            active.tool.deactivate(&mut active.context);
        }

        self.active_tool = tool;

        if let Some(mut active) = self.active_tool_mut() {
            active.tool.activate(&mut active.context);
        }

        Ok(())
    }

    /// Returns an overlay for read-only queries such as picking-key lookup.
    pub fn overlay(&self, key: MeshKey) -> Option<&TopologyOverlay> {
        self.overlays.get(key)
    }

    /// Returns this mesh's packed overlay buffers, or `None` for an unknown key.
    /// Mesh changes are applied during [`Self::update`].
    pub fn overlay_buffers(&self, key: MeshKey) -> Option<&OverlayBuffers> {
        Some(self.overlays.get(key)?.buffers())
    }

    fn cancel_active_tool(&mut self) {
        let Some(mut tool_accessor) = self.active_tool_mut() else {
            return;
        };

        tool_accessor.cancel();
    }

    fn update_active_tool_mode(&mut self, mode: EditMode) {
        let Some(mut tool_accessor) = self.active_tool_mut() else {
            return;
        };

        let compatible = tool_accessor.tool.supports_edit_mode(mode);

        tool_accessor.cancel();

        if compatible {
            return;
        }

        tool_accessor.tool.deactivate(&mut tool_accessor.context);
        self.active_tool = None;
    }

    fn validate_tool_activation(&self, tool: Option<ToolId>) -> Result<(), ToolActivationError> {
        let Some(id) = tool else {
            return Ok(());
        };

        let requested = self
            .tools
            .get(&id)
            .ok_or(ToolActivationError::NotFound { id })?;

        if !requested.supports_edit_mode(self.mode) {
            return Err(ToolActivationError::IncompatibleMode {
                id,
                mode: self.mode,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh-editor.rs"]
mod tests;
