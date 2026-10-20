//! JS adapters for tools.
//!
//! JS registers type-erased [`BoxedTool`]s (exported as `Tool`). Every crate that provides tools,
//! this one included, exports factories returning them, like `SelectTool.create()`, so the editor
//! never names concrete tool types. Plugin crates must be linked into the same `.wasm` as the
//! editor.

pub(super) mod input;
mod select;
mod translate;

use wasm_bindgen::convert::{FromWasmAbi, IntoWasmAbi};
use wasm_bindgen::describe::WasmDescribe;
use wasm_bindgen::prelude::*;

use crate::{Tool, ToolId};

/// A type-erased tool, ready to register with `MeshEditor.addTool`. Exported as `Tool`.
#[wasm_bindgen(js_name = Tool)]
pub struct BoxedTool {
    pub(super) tool: Box<dyn Tool>,
}

impl BoxedTool {
    /// Wraps a tool for JS. Any crate that provides tools (this one included, as well as
    /// plugins) calls this from its exported factories, like `SelectTool.create()`.
    pub fn new<ToolType: Tool>(tool: ToolType) -> Self {
        Self {
            tool: Box::new(tool),
        }
    }
}

// `ToolId` crosses the boundary as a JS number. See the `MeshKey` conversion for caveats.

impl WasmDescribe for ToolId {
    fn describe() {
        usize::describe();
    }
}

impl IntoWasmAbi for ToolId {
    type Abi = <usize as IntoWasmAbi>::Abi;

    fn into_abi(self) -> Self::Abi {
        self.raw().into_abi()
    }
}

impl FromWasmAbi for ToolId {
    type Abi = <usize as FromWasmAbi>::Abi;

    unsafe fn from_abi(js: Self::Abi) -> Self {
        // SAFETY: `js` came from JS as a `usize` ABI value, as declared by `describe`.
        ToolId::from_raw(unsafe { usize::from_abi(js) })
    }
}
