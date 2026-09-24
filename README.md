# mesh-editor

A modular mesh-editing library for the web and native. Use it headless, or bring your own renderer and wire it up to consume GPU-ready mesh data and shaders for rendering mesh points, edges, and faces.

## Workspace layout

| Crate | Role |
| --- | --- |
| `mesh` | Headless mesh kernel based on a radial edge structure. |
| `topology-overlay` | A translation layer from the mesh data model to GPU-ready data buffers for drawing the mesh overlay (points, edges, and faces). |
| `mesh-editor` | An orchestration layer for composing multiple meshes and overlays, and manager of global state (like edit mode, tools, etc.). |

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
wasm-pack build mesh-editor --target web --out-name mesh-editor -- --features wasm
```

This emits an npm-ready package (`.wasm` + JS glue + auto-generated `.d.ts`) into `mesh-editor/pkg/`, which CesiumJS imports.

## Docs

```sh
cargo doc --no-deps --document-private-items --open
```

## License

[Apache-2.0](LICENSE.md).
