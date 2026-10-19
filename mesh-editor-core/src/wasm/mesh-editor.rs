//! JS adapters for [`MeshEditor`] and the types in its module.

use std::ops::Range;

use serde::Serialize;
use slotmap::{Key, KeyData};
use topology_overlay::{OverlayBuffer, OverlayBuffers};
use tsify::{Ts, Tsify};
use wasm_bindgen::convert::{FromWasmAbi, IntoWasmAbi, VectorIntoWasmAbi};
use wasm_bindgen::describe::{WasmDescribe, WasmDescribeVector};
use wasm_bindgen::prelude::*;

use super::tool::BoxedTool;
use super::tool::input::{self, PointerInputJs};
use crate::{
    EditMode, MeshEditor, MeshKey, MeshNotFound, ToolActivationError, ToolId, UpdateResult,
};

// Doc comments on these JS-facing types also document the generated TypeScript.

/// The scalar type of each component in a buffer element.
#[derive(Clone, Copy, Serialize, Tsify)]
#[serde(rename_all = "lowercase")]
pub enum ScalarType {
    Float32,
    Uint32,
    /// An unsigned byte that shaders read as a normalized value: 255 is 1.0.
    Unorm8,
}

impl ScalarType {
    const fn size(self) -> usize {
        match self {
            Self::Float32 | Self::Uint32 => 4,
            Self::Unorm8 => 1,
        }
    }
}

/// The layout of one buffer element: `components` values of `scalarType`.
#[derive(Clone, Copy, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ElementLayout {
    scalar_type: ScalarType,
    components: usize,
}

/// An overlay buffer element type and its layout.
trait OverlayElement {
    const LAYOUT: ElementLayout;
}

impl OverlayElement for [f32; 4] {
    const LAYOUT: ElementLayout = ElementLayout {
        scalar_type: ScalarType::Float32,
        components: 4,
    };
}

impl OverlayElement for [u32; 2] {
    const LAYOUT: ElementLayout = ElementLayout {
        scalar_type: ScalarType::Uint32,
        components: 2,
    };
}

impl OverlayElement for [u32; 4] {
    const LAYOUT: ElementLayout = ElementLayout {
        scalar_type: ScalarType::Uint32,
        components: 4,
    };
}

impl OverlayElement for u8 {
    const LAYOUT: ElementLayout = ElementLayout {
        scalar_type: ScalarType::Unorm8,
        components: 1,
    };
}

const fn assert_layout_size<T: OverlayElement>() {
    assert!(T::LAYOUT.components * T::LAYOUT.scalar_type.size() == size_of::<T>());
}

// Fails to compile if a layout disagrees with its element type's size.
const _: () = {
    assert_layout_size::<[f32; 4]>();
    assert_layout_size::<[u32; 2]>();
    assert_layout_size::<[u32; 4]>();
    assert_layout_size::<u8>();
};

/// The location of one overlay buffer in the module's linear memory.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBufferView {
    /// Byte offset of the first element in `memory.buffer`, where `memory` is the module's
    /// `WebAssembly.Memory` (`InitOutput.memory`, from `init()` or `initSync()`).
    address: usize,
    /// Number of elements, each laid out as `layout` describes.
    /// A typed-array view over the buffer holds `elementCount * layout.components` values.
    element_count: usize,
    /// Element indices (not bytes) changed by the latest `update`, end-exclusive.
    /// Covers every change, so it may include unchanged elements in between.
    /// `undefined` when nothing changed or when `fullRefresh` is set.
    dirty_range: Option<Range<usize>>,
    layout: ElementLayout,
}

impl<T: OverlayElement> From<&OverlayBuffer<T>> for OverlayBufferView {
    fn from(buffer: &OverlayBuffer<T>) -> Self {
        Self {
            address: buffer.data.as_ptr() as usize,
            element_count: buffer.data.len(),
            dirty_range: buffer.dirty_range.clone(),
            layout: T::LAYOUT,
        }
    }
}

/// Selection buffers by component kind. One value per component: 0 unselected, 255 selected.
#[derive(Serialize, Tsify)]
pub struct OverlaySelectionViews {
    verts: OverlayBufferView,
    edges: OverlayBufferView,
    faces: OverlayBufferView,
}

/// Locations of a mesh's overlay buffers, for zero-copy typed-array views over wasm memory.
///
/// Valid only until the next call into the module: memory growth detaches `memory.buffer`,
/// topology edits reallocate the buffers, and `removeMesh` frees them.
/// Call `overlayViews` after `update`, read or upload right away, and don't keep views.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct OverlayViews {
    /// Upload every buffer in full. Dirty ranges are `undefined` when set.
    full_refresh: bool,
    /// `[x, y, z, 0]` per vertex.
    positions: OverlayBufferView,
    /// Two vertex indices per edge.
    edges: OverlayBufferView,
    /// Three vertex indices and a face index per triangle.
    triangles: OverlayBufferView,
    selection: OverlaySelectionViews,
}

impl From<&OverlayBuffers> for OverlayViews {
    fn from(buffers: &OverlayBuffers) -> Self {
        Self {
            full_refresh: buffers.full_refresh,
            positions: (&buffers.positions).into(),
            edges: (&buffers.edges).into(),
            triangles: (&buffers.triangles).into(),
            selection: OverlaySelectionViews {
                verts: (&buffers.selection.verts).into(),
                edges: (&buffers.selection.edges).into(),
                faces: (&buffers.selection.faces).into(),
            },
        }
    }
}

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

    /// Describes where a mesh's overlay buffers are in wasm memory and what the latest
    /// [`MeshEditor::update`] changed. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = overlayViews)]
    pub fn overlay_views(&self, key: MeshKey) -> Result<Ts<OverlayViews>, JsError> {
        let buffers = self.overlay_buffers(key).ok_or(MeshNotFound { key })?;

        Ok(OverlayViews::from(buffers).into_ts()?)
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
    pub fn tool_pointer_down(&mut self, input: Ts<PointerInputJs>) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_down(input))
    }

    /// Forwards a pointer release to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerUp)]
    pub fn tool_pointer_up(&mut self, input: Ts<PointerInputJs>) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_up(input))
    }

    /// Forwards pointer movement to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerMove)]
    pub fn tool_pointer_move(&mut self, input: Ts<PointerInputJs>) -> Result<bool, JsError> {
        input::dispatch_pointer(self, input, |tool, input| tool.pointer_move(input))
    }

    /// Forwards a click to the active tool. Returns whether it was handled.
    #[wasm_bindgen(js_name = pointerClick)]
    pub fn tool_pointer_click(&mut self, input: Ts<PointerInputJs>) -> Result<bool, JsError> {
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
