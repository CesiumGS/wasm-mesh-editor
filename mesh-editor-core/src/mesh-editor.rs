use std::collections::HashMap;

use mesh_core::Mesh;
use slotmap::{SecondaryMap, SlotMap, new_key_type};
use topology_overlay::{OverlayBuffers, TopologyOverlay};

use crate::{EditMode, Tool, ToolAccessor, ToolHandle, ToolId};

#[path = "mesh-editor/errors.rs"]
mod errors;

pub use errors::MeshNotFound;

new_key_type! {
    /// Identifies a mesh registered with an editor.
    pub struct MeshKey;
}

/// Owns registered meshes and coordinates their overlays and active tool.
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

#[allow(unused_variables)]
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

    /// Applies the mode's selection level to active meshes.
    pub fn set_mode(&mut self, mode: EditMode) {
        if self.mode == mode {
            return;
        }

        self.mode = mode;

        for &key in &self.active_meshes {
            self.meshes[key]
                .selection_mut()
                .set_level(mode.selection_level);
        }
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
        let mesh = self.meshes.remove(key)?;

        self.overlays.remove(key);
        self.mesh_keys.retain(|&mesh_key| mesh_key != key);
        self.active_meshes.retain(|&mesh_key| mesh_key != key);

        Some(mesh)
    }

    pub fn has_mesh(&self, key: MeshKey) -> bool {
        self.meshes.contains_key(key)
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

    pub fn is_mesh_active(&self, key: MeshKey) -> bool {
        self.active_meshes.contains(&key)
    }

    /// Applies the current mode to this mesh. Returns [`MeshNotFound`] for an unknown key.
    pub fn set_mesh_active(&mut self, key: MeshKey) -> Result<(), MeshNotFound> {
        let mesh = self.meshes.get_mut(key).ok_or(MeshNotFound { key })?;

        if self.active_meshes.contains(&key) {
            return Ok(());
        }

        mesh.selection_mut().set_level(self.mode.selection_level);
        self.active_meshes.push(key);

        Ok(())
    }

    /// Stops editing this mesh without changing its selection or selection level.
    /// Returns [`MeshNotFound`] for an unknown key.
    pub fn set_mesh_inactive(&mut self, key: MeshKey) -> Result<(), MeshNotFound> {
        self.meshes.get(key).ok_or(MeshNotFound { key })?;

        self.active_meshes.retain(|&mesh_key| mesh_key != key);

        Ok(())
    }

    /// Registers a tool with this editor. The tool starts inactive.
    pub fn add_tool<ToolType: Tool>(&mut self, tool: ToolType) -> ToolHandle<ToolType> {
        let id = ToolId::new();

        self.tools.insert(id, Box::new(tool));

        ToolHandle::new(id)
    }

    pub fn active_tool(&self) -> Option<ToolId> {
        self.active_tool
    }

    /// Borrows the active tool and constructs its mesh context.
    pub fn active_tool_mut(&mut self) -> Option<ToolAccessor<'_>> {
        todo!()
    }

    /// Borrows a tool only if its ID belongs to this editor and it is active.
    /// Retained handles use this to avoid dispatching to a different tool.
    pub fn tool_mut(&mut self, id: ToolId) -> Option<ToolAccessor<'_>> {
        todo!()
    }

    /// Borrows a concrete tool for configuration, active or inactive.
    /// Returns `None` for a foreign or missing handle.
    ///
    /// ```no_run
    /// use mesh_editor_core::{MeshEditor, Tool};
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
    /// Returns `None` if the ID does not belong to this editor.
    pub fn set_active_tool(&mut self, tool: Option<ToolId>) -> Option<()> {
        todo!()
    }

    /// Advances the active tool and updates overlays. Called by the host's frame loop.
    pub fn update(&mut self, delta_seconds: f64) {
        todo!()
    }

    /// Returns an overlay for read-only queries such as picking-key lookup.
    pub fn overlay(&self, key: MeshKey) -> Option<&TopologyOverlay> {
        self.overlays.get(key)
    }

    /// Returns this mesh's overlay changes from the latest update, if any.
    /// Consume borrowed data before the next update.
    pub fn overlay_buffers(&self, key: MeshKey) -> Option<&OverlayBuffers> {
        todo!()
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh-editor.rs"]
mod tests;
