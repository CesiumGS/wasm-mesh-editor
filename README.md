# mesh-editor

A modular mesh-editing library for the web and native. Use it headless, or bring your own renderer and wire it up to consume GPU-ready mesh data and shaders for rendering mesh points, edges, and faces.

## Workspace layout

| Crate | Role |
| --- | --- |
| `mesh-core` | Headless mesh kernel based on a radial edge structure. |
| `topology-overlay` | A translation layer from the mesh data model to GPU-ready data buffers for drawing the mesh overlay (points, edges, and faces). |
| `mesh-editor-core` | An orchestration layer for composing multiple meshes and overlays, and manager of global state (like edit mode, tools, etc.). With the `wasm` feature, it also compiles everything into one `.wasm` and exposes the editor to JS via `wasm-bindgen`. |

Small, reusable utility crates live under `utils/`:

| Crate | Role |
| --- | --- |
| [event-emitter](utils/event-emitter/) | Queued broadcasts with shared read-only payloads and receiver-owned subscriptions. |

## Prerequisites

- [Rust](https://rustup.rs/) (stable; edition 2024 needs 1.85+)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) 22+ (to consume/test the package)

The `rust-toolchain.toml` pins the toolchain and adds the `wasm32-unknown-unknown` target automatically.

## Develop and test (native — fast)

```sh
cargo test          # runs unit tests for native crates
cargo clippy        # lints
cargo fmt           # format
```

## Build the Wasm package

```sh
wasm-pack build mesh-editor-core --target web --out-name mesh-editor -- --features wasm
```

This emits an npm-ready package (`.wasm` + JS glue + auto-generated `.d.ts`) into `mesh-editor-core/pkg/`, which CesiumJS imports.

JS bindings are declared with `#[cfg_attr(feature = "wasm", wasm_bindgen)]` beside the Rust items they expose. Key conversions, JS-only types, and adapter methods live under each crate's `src/wasm/`, in modules named after the source modules they adapt (e.g. `mesh-core/src/wasm/mesh.rs` adapts `mesh.rs`). The `wasm` feature of `mesh-editor-core` also enables `mesh-core/wasm`. Native builds never compile `wasm-bindgen`.

JS registers tools without the editor naming concrete types: `editor.addTool(SelectTool.create())`. A crate that ships more tools exports its own factory returning a `BoxedTool` (`Tool` in JS), built with `BoxedTool::new(MyTool::default())`. Tool crates must be compiled into the same `.wasm` as the editor, so a build that includes extra tool crates needs a top-level crate that depends on `mesh-editor-core` and on them.

Add `--dev` for a faster, unoptimized build.

## Docs

```sh
cargo doc --no-deps --document-private-items --open
```

## License

[Apache-2.0](LICENSE.md).
