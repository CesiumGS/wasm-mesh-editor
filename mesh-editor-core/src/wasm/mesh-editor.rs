//! JS adapters for [`MeshEditor`] and the types in its module.

use slotmap::{Key, KeyData};
use wasm_bindgen::convert::{FromWasmAbi, IntoWasmAbi, VectorIntoWasmAbi};
use wasm_bindgen::describe::{WasmDescribe, WasmDescribeVector};
use wasm_bindgen::prelude::*;

use super::tool::{BoxedTool, input};
use crate::{
    EditMode, MeshEditor, MeshKey, MeshNotFound, ToolActivationError, ToolId, UpdateResult,
};

// `MeshKey` crosses the boundary as its slotmap FFI value, a `u64` (a JS `bigint`).
// These traits are wasm-bindgen's documented-but-unstable conversion API; a breaking
// change would surface as a compile error here.

impl WasmDescribe for MeshKey {
    fn describe() {
        u64::describe();
    }
}

impl IntoWasmAbi for MeshKey {
    type Abi = <u64 as IntoWasmAbi>::Abi;

    fn into_abi(self) -> Self::Abi {
        self.data().as_ffi().into_abi()
    }
}

impl FromWasmAbi for MeshKey {
    type Abi = <u64 as FromWasmAbi>::Abi;

    unsafe fn from_abi(js: Self::Abi) -> Self {
        // SAFETY: `js` came from JS as a `u64` ABI value, as declared by `describe`.
        MeshKey::from(KeyData::from_ffi(unsafe { u64::from_abi(js) }))
    }
}

impl WasmDescribeVector for MeshKey {
    fn describe_vector() {
        u64::describe_vector();
    }
}

impl VectorIntoWasmAbi for MeshKey {
    type Abi = <Box<[u64]> as IntoWasmAbi>::Abi;

    fn vector_into_abi(vector: Box<[Self]>) -> Self::Abi {
        let keys: Box<[u64]> = vector.iter().map(|key| key.data().as_ffi()).collect();

        keys.into_abi()
    }
}

impl From<MeshNotFound> for JsValue {
    fn from(error: MeshNotFound) -> Self {
        JsError::from(error).into()
    }
}

impl From<ToolActivationError> for JsValue {
    fn from(error: ToolActivationError) -> Self {
        JsError::from(error).into()
    }
}

#[wasm_bindgen]
impl UpdateResult {
    /// Creates an empty result to reuse across [`MeshEditor::update`] calls.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
}

#[wasm_bindgen]
impl MeshEditor {
    /// Copies the registered mesh keys, since [`MeshEditor::mesh_keys`] returns a borrowed slice.
    #[wasm_bindgen(js_name = meshKeys)]
    pub fn mesh_keys_owned(&self) -> Vec<MeshKey> {
        self.mesh_keys().to_vec()
    }

    /// Copies the active mesh keys, since [`MeshEditor::active_meshes`] returns a borrowed slice.
    #[wasm_bindgen(js_name = activeMeshes)]
    pub fn active_meshes_owned(&self) -> Vec<MeshKey> {
        self.active_meshes().to_vec()
    }

    /// Applies the mode's selection level to active meshes and deactivates incompatible tools.
    /// Takes the mode by reference so JS can pass the same `EditMode` object repeatedly.
    #[wasm_bindgen(js_name = setMode)]
    pub fn set_mode_by_ref(&mut self, mode: &EditMode) {
        self.set_mode(*mode);
    }

    /// Registers a tool, taking ownership of it. The tool starts inactive.
    #[wasm_bindgen(js_name = addTool)]
    pub fn add_tool_object(&mut self, tool: BoxedTool) -> ToolId {
        self.add_boxed_tool(tool.tool)
    }

    /// The active tool's ID, or `undefined` if no tool is active.
    #[wasm_bindgen(getter = activeTool)]
    pub fn active_tool_id(&self) -> Option<usize> {
        self.active_tool().map(ToolId::raw)
    }

    /// Activates a registered tool, cancelling the current one's interaction.
    /// Throws for an unknown tool or one that doesn't support the current mode.
    #[wasm_bindgen(js_name = setActiveTool)]
    pub fn activate_tool(&mut self, id: ToolId) -> Result<(), ToolActivationError> {
        self.set_active_tool(Some(id))
    }

    /// Deactivates the active tool, cancelling its interaction.
    #[wasm_bindgen(js_name = clearActiveTool)]
    pub fn clear_active_tool(&mut self) -> Result<(), ToolActivationError> {
        self.set_active_tool(None)
    }

    /// Forwards a pointer press to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerDown)]
    pub fn tool_pointer_down(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "PointerInput")] input: JsValue,
    ) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_down(input))
    }

    /// Forwards a pointer release to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerUp)]
    pub fn tool_pointer_up(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "PointerInput")] input: JsValue,
    ) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_up(input))
    }

    /// Forwards pointer movement to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerMove)]
    pub fn tool_pointer_move(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "PointerInput")] input: JsValue,
    ) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_move(input))
    }

    /// Forwards a click to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerClick)]
    pub fn tool_pointer_click(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "PointerInput")] input: JsValue,
    ) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_click(input))
    }

    /// Applies any pending preview and confirms the active tool's current interaction.
    #[wasm_bindgen(js_name = confirm)]
    pub fn confirm_tool(&mut self) {
        if let Some(mut tool) = self.active_tool_mut() {
            tool.confirm();
        }
    }

    /// Cancels the active tool's current interaction.
    #[wasm_bindgen(js_name = cancel)]
    pub fn cancel_tool(&mut self) {
        if let Some(mut tool) = self.active_tool_mut() {
            tool.cancel();
        }
    }
}
