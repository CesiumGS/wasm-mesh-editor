# mesh-editor

A modular mesh-editing library for the web and native. Use it headless, or bring your own renderer and wire it up to consume GPU-ready mesh data and shaders for rendering mesh points, edges, and faces.

## Workspace layout

| Crate | Role |
| --- | --- |
| `mesh` | Headless mesh kernel based on a radial edge structure. |
| `topology-overlay` | A translation layer from the mesh data model to GPU-ready data buffers for drawing the mesh overlay (points, edges, and faces), plus the WebGL2 shaders that draw them. |
| `mesh-editor` | An orchestration layer for composing multiple meshes and overlays, and manager of global state (like edit mode, tools, etc.). With the `wasm` feature, it also compiles everything into one `.wasm` and exposes the editor to JS via `wasm-bindgen`. |

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
wasm-pack build mesh-editor --target web --scope cesium --out-name mesh-editor -- --features wasm
```

This emits an npm-ready package (`.wasm` + JS glue + auto-generated `.d.ts`) into `mesh-editor/pkg/`, which CesiumJS imports. The `--scope cesium` flag names the package `@cesium/mesh-editor`.

While the API is unstable the package ships as pre-releases versioned below `1.0.0` (`0.1.0-alpha.N`, set by the workspace `version` in `Cargo.toml`). To publish one, from `mesh-editor/pkg/` run:

```sh
npm publish --access public --tag alpha
```

`--access public` is required to publish a scoped package, and `--tag alpha` places it under the `alpha` dist-tag instead of `latest`, so pre-releases aren't installed by default. Publishing to the `@cesium` scope requires npm org membership.

JS bindings are declared with `#[cfg_attr(feature = "wasm", wasm_bindgen)]` beside the Rust items they expose. Key conversions, JS-only types, and adapter methods live under each crate's `src/wasm/`, in modules named after the source modules they adapt (e.g. `mesh/src/wasm/mesh.rs` adapts `mesh.rs`). The `wasm` feature of `mesh-editor` also enables `mesh/wasm`. Native builds never compile `wasm-bindgen`.

Plain JS objects that cross the boundary (such as `PointerInput` and `OverlayViews`) are serde structs deriving [`Tsify`](https://github.com/madonoharu/tsify), passed as `tsify::Ts<T>`, so their TypeScript interfaces and doc comments are generated from Rust. Their Rust names become TypeScript names, and their doc comments become TypeScript docs, so keep both JS-facing. Define them in `mesh-editor`, the crate that builds the `.wasm`: a dependency crate's declarations can be dropped from dev builds' `.d.ts`, since nothing links the object file that holds them.

JS registers tools without the editor naming concrete types: `editor.addTool(SelectTool.create())`. A crate that ships more tools exports its own factory returning a `BoxedTool` (`Tool` in JS), built with `BoxedTool::new(MyTool::default())`. Tool crates must be compiled into the same `.wasm` as the editor, so a build that includes extra tool crates needs a top-level crate that depends on `mesh-editor` and on them.

Add `--dev` for a faster, unoptimized build.

## Overlay shaders

`topology-overlay/shaders/` holds GLSL ES 3.00 shaders that draw the overlay buffers as WebGL2 data textures, one instanced draw per component kind. They're embedded in the `.wasm`: JS reads them from `topologyOverlayShaders()`.

Each shader owns its `main()` and calls hook functions that the consumer defines, so camera transforms, depth writing (such as log depth), gamma correction, and pick encoding stay on the consumer's side:

| Stage | Hook |
| --- | --- |
| Vertex | `vec4 topologyOverlay_modelToEye(vec3 positionMC)` |
| Vertex | `vec4 topologyOverlay_eyeToClip(vec4 positionEC)` |
| Vertex | `vec2 topologyOverlay_viewportSize()` |
| Vertex | `void topologyOverlay_setVertexOutputs(vec4 positionEC, uint componentIndex)` |
| Fragment | `void topologyOverlay_writeColor(vec4 color)` |

`topologyOverlay_eyeToClip` must be linear, like a projection matrix, and free of side effects: edges call it twice per vertex and interpolate its results to clip to the near plane. `topologyOverlay_setVertexOutputs` is called once per vertex after `gl_Position` is final, with the vertex's eye position and the drawn component's overlay index: the place for custom depth (it may adjust `gl_Position`'s depth, as Cesium's `czm_vertexLogDepth()` does) and pick colors.

Build each stage as `#version 300 es`, then the shader's source, then the hook definitions. `defaultHooks` defines them for plain WebGL2. Bind each overlay buffer's texture to its `layout.uniformName` from `overlayViews`; `topologyOverlayShaders().uniforms` names the rest, such as each shader's sizes and colors, the shared depth bias, and the view uniforms that the default hooks declare. Point and edge sizes are in the pixels of `topologyOverlay_viewportSize()`, so scale them by the device pixel ratio. Upload each buffer as a texture with `NEAREST` min and mag filters: the default filters leave data textures incomplete, and the shaders then read zeros.

## Docs

```sh
cargo doc --no-deps --document-private-items --open
```

## License

[Apache-2.0](LICENSE.md).
