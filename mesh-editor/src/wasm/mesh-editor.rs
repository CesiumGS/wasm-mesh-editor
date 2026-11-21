//! JS adapters for [`MeshEditor`] and the types in its module.

#[path = "mesh-editor/attributes.rs"]
mod attributes;
#[path = "mesh-editor/selection.rs"]
mod selection;

use std::ops::Range;

use glam::Vec3;
use mesh::{ComponentKey, EdgeKey, FaceKey, VertKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use slotmap::{Key, KeyData};
use topology_overlay::shaders::uniforms;
use topology_overlay::{OverlayBuffer, OverlayBuffers};
use tsify::{Ts, Tsify};
use wasm_bindgen::convert::{FromWasmAbi, IntoWasmAbi, VectorIntoWasmAbi};
use wasm_bindgen::describe::{WasmDescribe, WasmDescribeVector};
use wasm_bindgen::prelude::*;

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

/// How to read one overlay buffer: each element is `components` values of `scalarType`, uploaded
/// as one texel of the texture that the shaders read from `uniformName`.
#[derive(Clone, Copy, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct BufferLayout {
    scalar_type: ScalarType,
    components: usize,
    uniform_name: &'static str,
}

/// An overlay buffer element type: `COMPONENTS` values of `SCALAR_TYPE`.
trait OverlayElement {
    const SCALAR_TYPE: ScalarType;
    const COMPONENTS: usize;
}

impl OverlayElement for [f32; 4] {
    const SCALAR_TYPE: ScalarType = ScalarType::Float32;
    const COMPONENTS: usize = 4;
}

impl OverlayElement for [u32; 2] {
    const SCALAR_TYPE: ScalarType = ScalarType::Uint32;
    const COMPONENTS: usize = 2;
}

impl OverlayElement for [u32; 4] {
    const SCALAR_TYPE: ScalarType = ScalarType::Uint32;
    const COMPONENTS: usize = 4;
}

impl OverlayElement for u8 {
    const SCALAR_TYPE: ScalarType = ScalarType::Unorm8;
    const COMPONENTS: usize = 1;
}

const fn assert_layout_size<T: OverlayElement>() {
    assert!(T::COMPONENTS * T::SCALAR_TYPE.size() == size_of::<T>());
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
    layout: BufferLayout,
}

impl OverlayBufferView {
    fn new<T: OverlayElement>(buffer: &OverlayBuffer<T>, uniform_name: &'static str) -> Self {
        Self {
            address: buffer.data.as_ptr() as usize,
            element_count: buffer.data.len(),
            dirty_range: buffer.dirty_range.clone(),
            layout: BufferLayout {
                scalar_type: T::SCALAR_TYPE,
                components: T::COMPONENTS,
                uniform_name,
            },
        }
    }
}

/// The location of one component type's keys in the module's linear memory, like
/// `OverlayBufferView`. Read them with a `BigUint64Array`.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct OverlayKeyView {
    address: usize,
    element_count: usize,
}

impl OverlayKeyView {
    fn new<K>(keys: &[K]) -> Self {
        // Mesh component keys are `repr(transparent)` over `u64`s.
        const { assert!(size_of::<K>() == 8 && align_of::<K>() == 8) };

        Self {
            address: keys.as_ptr() as usize,
            element_count: keys.len(),
        }
    }
}

/// Component keys in buffer order, to pass as `PickHit.componentKey`. `faces` is indexed by each triangle's
/// face index, not by triangle.
#[derive(Serialize, Tsify)]
pub struct OverlayKeyViews {
    verts: OverlayKeyView,
    edges: OverlayKeyView,
    faces: OverlayKeyView,
}

/// Component keys grouped by type, like `OverlayViews.componentKeys`.
/// If a component type is not specified, it's treated as having no keys.
#[derive(Serialize, Deserialize, Tsify, Default)]
#[serde(default)]
#[tsify(rename = "ComponentKeys")]
pub struct ComponentKeysJs {
    #[serde(with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    verts: Vec<u64>,
    #[serde(with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    edges: Vec<u64>,
    #[serde(with = "big_uint64_array")]
    #[tsify(optional, type = "BigUint64Array")]
    faces: Vec<u64>,
}

impl ComponentKeysJs {
    /// Converts JS keys, dropping any that no slot map could have issued.
    fn to_component_keys(component_keys: &Ts<Self>) -> Result<Vec<ComponentKey>, JsError> {
        // `Tsify::from_js` rather than `Ts::to_rust`, whose errors name internal Rust types.
        Ok(Self::from_js(component_keys.js_value())?.into_component_keys())
    }

    /// Like `to_component_keys`, for keys already converted from JS.
    pub(in crate::wasm) fn into_component_keys(self) -> Vec<ComponentKey> {
        let verts = self
            .verts
            .into_iter()
            .filter_map(|key| VertKey::try_from_ffi(key).map(ComponentKey::from));
        let edges = self
            .edges
            .into_iter()
            .filter_map(|key| EdgeKey::try_from_ffi(key).map(ComponentKey::from));
        let faces = self
            .faces
            .into_iter()
            .filter_map(|key| FaceKey::try_from_ffi(key).map(ComponentKey::from));

        verts.chain(edges).chain(faces).collect()
    }
}

/// Selection buffers by component kind. One value per component: 0 unselected, 255 selected.
/// Each binds to the same uniform, in the draw for its component kind.
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
    /// Changes only when `fullRefresh` is set.
    component_keys: OverlayKeyViews,
}

impl OverlayViews {
    fn new(buffers: &OverlayBuffers, full_refresh: bool) -> Self {
        Self {
            full_refresh,
            positions: OverlayBufferView::new(&buffers.positions, uniforms::POSITION_TEXTURE),
            edges: OverlayBufferView::new(&buffers.edges, uniforms::EDGE_TEXTURE),
            triangles: OverlayBufferView::new(&buffers.triangles, uniforms::TRIANGLE_TEXTURE),
            selection: OverlaySelectionViews {
                verts: OverlayBufferView::new(
                    &buffers.selection.verts,
                    uniforms::SELECTION_TEXTURE,
                ),
                edges: OverlayBufferView::new(
                    &buffers.selection.edges,
                    uniforms::SELECTION_TEXTURE,
                ),
                faces: OverlayBufferView::new(
                    &buffers.selection.faces,
                    uniforms::SELECTION_TEXTURE,
                ),
            },
            component_keys: OverlayKeyViews {
                verts: OverlayKeyView::new(&buffers.component_keys.verts),
                edges: OverlayKeyView::new(&buffers.component_keys.edges),
                faces: OverlayKeyView::new(&buffers.component_keys.faces),
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
        let overlay = self.overlay(key).ok_or(MeshNotFound { key })?;

        Ok(OverlayViews::new(overlay.buffers(), overlay.full_refresh()).into_ts()?)
    }

    /// Writes a mesh's local-space bounds to `result` as `[minX, minY, minZ, maxX, maxY, maxZ]`.
    /// Returns `false` and leaves `result` unchanged for a mesh with no vertices.
    /// Throws for an unknown mesh key or a `result` whose length isn't 6.
    #[wasm_bindgen(js_name = meshBounds)]
    pub fn mesh_bounds(
        &self,
        key: MeshKey,
        result: &js_sys::Float64Array,
    ) -> Result<bool, JsError> {
        let mesh = self.mesh(key).ok_or(MeshNotFound { key })?;
        let length = result.length();

        if length != 6 {
            return Err(JsError::new(&format!("result length {length} is not 6")));
        }

        let Some(bounds) = mesh.bounds() else {
            return Ok(false);
        };

        let (min, max) = (bounds.min.as_dvec3(), bounds.max.as_dvec3());

        // A JS-owned output avoids wasm allocations
        result.copy_from(&[min.x, min.y, min.z, max.x, max.y, max.z]);

        Ok(true)
    }

    /// Moves an active mesh's selected vertices by a local-space delta. Returns `false` and does
    /// nothing if the mesh is inactive. Throws for an unknown mesh key.
    #[wasm_bindgen(js_name = translateSelected)]
    pub fn translate_selected(
        &mut self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<bool, JsError> {
        let Some(mesh) = active_mesh(self, mesh_key)? else {
            return Ok(false);
        };

        mesh.translate_selected(Vec3::new(x, y, z));
        Ok(true)
    }

    /// Makes `components` selectable.
    /// Applies the selection level to active meshes and deactivates incompatible tools.
    /// Throws for bits outside `ComponentMask`.
    #[wasm_bindgen(js_name = setMode)]
    pub fn set_mode_from_components(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "ComponentMask")] components: u32,
    ) -> Result<(), JsError> {
        self.set_mode(EditMode::from_js_components(components)?);

        Ok(())
    }

    /// The components the renderer should draw in the current mode.
    #[wasm_bindgen(getter = renderableComponents, unchecked_return_type = "ComponentMask")]
    pub fn renderable_components(&self) -> u8 {
        self.mode().renderable_components.bits()
    }

    /// The components that can be selected directly in the current mode.
    #[wasm_bindgen(getter = selectableComponents, unchecked_return_type = "ComponentMask")]
    pub fn selectable_components(&self) -> u8 {
        self.mode().selection_level.bits()
    }

    /// The active tool's ID, or `undefined` if no tool is active.
    #[wasm_bindgen(getter = activeTool)]
    pub fn active_tool_id(&self) -> Option<usize> {
        self.active_tool().map(ToolId::raw)
    }

    /// Whether a registered tool, active or inactive, can be active with `components` selectable.
    /// Does not change the active tool. Throws for an unknown tool or bits outside `ComponentMask`.
    #[wasm_bindgen(js_name = toolSupportsMode)]
    pub fn tool_supports_mode(
        &self,
        id: ToolId,
        #[wasm_bindgen(unchecked_param_type = "ComponentMask")] components: u32,
    ) -> Result<bool, JsError> {
        let mode = EditMode::from_js_components(components)?;
        let tool = self.tool(id).ok_or(ToolActivationError::NotFound { id })?;

        Ok(tool.supports_edit_mode(mode))
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

    /// Cancels the active tool's current interaction. Selection or position edits made from JS
    /// during the interaction can make the cancel restore the wrong components.
    #[wasm_bindgen(js_name = cancel)]
    pub fn cancel_tool(&mut self) {
        if let Some(mut tool) = self.active_tool_mut() {
            tool.cancel();
        }
    }
}

fn ffi(key: impl Key) -> u64 {
    key.data().as_ffi()
}

/// The `K` key whose FFI value is `id`, or `None` if no slot map could have issued it.
fn key_from_ffi<K: Key>(id: u64) -> Option<K> {
    let key = K::from(KeyData::from_ffi(id));
    (key.data().as_ffi() == id).then_some(key)
}

/// The mesh to edit, or `None` if it's inactive. Throws for an unknown mesh key.
fn active_mesh(
    editor: &mut MeshEditor,
    mesh_key: MeshKey,
) -> Result<Option<&mut mesh::Mesh>, JsError> {
    if !editor.has_mesh(mesh_key) {
        return Err(MeshNotFound { key: mesh_key }.into());
    }

    Ok(editor.active_mesh_mut(mesh_key))
}

mod big_uint64_array {
    use super::*;

    pub fn serialize<S: Serializer>(keys: &[u64], serializer: S) -> Result<S::Ok, S::Error> {
        serde_wasm_bindgen::preserve::serialize(&js_sys::BigUint64Array::from(keys), serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u64>, D::Error> {
        let array: js_sys::BigUint64Array =
            serde_wasm_bindgen::preserve::deserialize(deserializer)?;

        Ok(array.to_vec())
    }
}
