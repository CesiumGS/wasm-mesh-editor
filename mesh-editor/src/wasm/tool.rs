//! JS adapters for tools.
//!
//! Tools stay owned by the editor; JS only receives `ToolId`s. Every crate that provides tools,
//! this one included, exports static functions on its tool types that take the editor, like
//! `SelectTool.addTo(editor)`, so the editor never names concrete tool types. Plugin crates must
//! be linked into the same `.wasm` as the editor.

pub(super) mod input;
mod select;
mod translate;

use wasm_bindgen::convert::{FromWasmAbi, IntoWasmAbi};
use wasm_bindgen::describe::WasmDescribe;

use crate::ToolId;

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
