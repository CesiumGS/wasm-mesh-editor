//! JS-only additions, compiled with the `wasm` feature.
//!
//! Most of the JS API is declared with `#[cfg_attr(feature = "wasm", wasm_bindgen)]` next to the
//! Rust code it exposes. These modules hold what can't be: ABI conversions, JS-only types, and
//! adapter methods for Rust signatures JS can't call directly. Each module adapts the source
//! module of the same name.

mod edit_mode;
#[path = "wasm/mesh-editor.rs"]
mod mesh_editor;
mod tool;

pub use tool::BoxedTool;
