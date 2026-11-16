// Compiles every topology overlay shader and renders real overlay data with them in
// headless Chromium's WebGL2. Build the wasm package first:
// `wasm-pack build mesh-editor --dev --target web --out-name mesh-editor -- --features wasm`.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

import init, {
  AttributeComparison,
  AttributeComponentType,
  AttributeDomain,
  ComponentType,
  CustomAttribute,
  Mesh,
  MeshEditor,
  SelectAction,
  SelectTool,
  SelectionStep,
  TranslateTool,
  UpdateResult,
  AttributeResult,
  topologyOverlayShaders,
} from "../../mesh-editor/pkg/mesh-editor.js";

const { memory } = await init({
  module_or_path: await readFile(
    new URL("../../mesh-editor/pkg/mesh-editor_bg.wasm", import.meta.url),
  ),
});

const shaders = topologyOverlayShaders();
const { uniforms } = shaders;
const CANVAS_SIZE = 64;

// Active uniforms per shader: unused ones (such as `u_viewportSize` for faces) are optimized out.
const sharedUniforms = [
  "u_modelView",
  "u_projection",
  "u_positionTexture",
  "u_flagTexture",
  "u_depthBias",
];

const activeUniforms = {
  point: [
    ...sharedUniforms,
    "u_viewportSize",
    "u_pointSize",
    "u_pointColor",
    "u_pointSelectedColor",
  ],
  edge: [
    ...sharedUniforms,
    "u_viewportSize",
    "u_edgeTexture",
    "u_edgeWidth",
    "u_edgeColor",
    "u_edgeSelectedColor",
  ],
  face: [
    ...sharedUniforms,
    "u_triangleTexture",
    "u_faceColor",
    "u_faceSelectedColor",
    "u_isPickPass",
  ],
};

let browser;
let page;

before(async () => {
  browser = await chromium.launch();
  page = await browser.newPage();
  await page.setContent(`<canvas width="${CANVAS_SIZE}" height="${CANVAS_SIZE}"></canvas>`);
  await page.evaluate(installPageHelpers);
});

after(async () => {
  await browser?.close();
});

/** Defines `buildShader` and `renderShader` in the page. */
function installPageHelpers() {
  const gl = document.querySelector("canvas").getContext("webgl2");

  // Compiles a shader's complete stage sources and links them into a WebGL program.
  window.buildShader = ({ vertex, fragment }) => {
    if (!gl) {
      return { error: "WebGL2 is unavailable" };
    }

    const compile = (type, source) => {
      const shader = gl.createShader(type);
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      return { shader, log: gl.getShaderInfoLog(shader) };
    };
    const vs = compile(gl.VERTEX_SHADER, vertex);
    const fs = compile(gl.FRAGMENT_SHADER, fragment);

    const program = gl.createProgram();
    gl.attachShader(program, vs.shader);
    gl.attachShader(program, fs.shader);
    gl.linkProgram(program);

    const linked = gl.getProgramParameter(program, gl.LINK_STATUS);
    const uniformCount = linked ? gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS) : 0;
    const uniforms = [];
    for (let i = 0; i < uniformCount; i++) {
      uniforms.push(gl.getActiveUniform(program, i).name);
    }

    return {
      program,
      linked,
      vertexLog: vs.log,
      fragmentLog: fs.log,
      linkLog: gl.getProgramInfoLog(program),
      uniforms,
    };
  };

  // Draws one instanced shader over a black canvas and returns its RGBA pixels, bottom row first.
  window.renderShader = ({ sources, textures, uniforms, mode, vertexCount, instanceCount }) => {
    const compiled = window.buildShader(sources);
    if (!compiled.linked) {
      return { error: compiled.linkLog || compiled.vertexLog || compiled.fragmentLog };
    }

    const { program } = compiled;
    gl.useProgram(program);
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.disable(gl.BLEND);
    gl.disable(gl.DEPTH_TEST);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);

    textures.forEach((texture, unit) => {
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl[texture.internalFormat],
        texture.width,
        texture.height,
        0,
        gl[texture.format],
        gl[texture.type],
        new self[texture.arrayType](texture.data),
      );
      // Required: the default filters leave data textures incomplete (no mipmaps, and 32-bit
      // float and integer formats can't be filtered linearly), and texelFetch then returns 0.
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.uniform1i(gl.getUniformLocation(program, texture.uniform), unit);
    });

    for (const [name, { type, value }] of Object.entries(uniforms)) {
      // A missing name in `topologyOverlayShaders().uniforms` becomes the key "undefined".
      if (name === "undefined") {
        return { error: "a uniform name is undefined" };
      }
      const location = gl.getUniformLocation(program, name);
      switch (type) {
        case "float":
          gl.uniform1f(location, value);
          break;
        case "bool":
          gl.uniform1i(location, value ? 1 : 0);
          break;
        case "vec2":
          gl.uniform2fv(location, value);
          break;
        case "vec4":
          gl.uniform4fv(location, value);
          break;
        case "mat4":
          gl.uniformMatrix4fv(location, false, value);
          break;
      }
    }

    gl.drawArraysInstanced(gl[mode], 0, vertexCount, instanceCount);

    const pixels = new Uint8Array(gl.drawingBufferWidth * gl.drawingBufferHeight * 4);
    gl.readPixels(
      0,
      0,
      gl.drawingBufferWidth,
      gl.drawingBufferHeight,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      pixels,
    );

    return { error: gl.getError() || undefined, pixels: Array.from(pixels) };
  };
}

/** Prepends the version and appends the default hooks, as the shader docs describe. */
function withDefaultHooks(name) {
  const sources = shaders[name];
  assert.ok(sources, `topologyOverlayShaders() has no ${name} shader`);

  return {
    vertex: `#version 300 es\n${sources.vertex}\n${shaders.defaultHooks.vertex}`,
    fragment: `#version 300 es\n${sources.fragment}\n${shaders.defaultHooks.fragment}`,
  };
}

/** Maps an overlay element layout to a WebGL2 texture format, as a consumer's adaptor would. */
function textureFormat({ scalarType, components }) {
  const channels = ["RED", "RG", "RGB", "RGBA"][components - 1];

  switch (scalarType) {
    case "float32":
      return {
        internalFormat: ["R32F", "RG32F", "RGB32F", "RGBA32F"][components - 1],
        format: channels,
        type: "FLOAT",
        arrayType: "Float32Array",
      };
    case "uint32":
      return {
        internalFormat: ["R32UI", "RG32UI", "RGB32UI", "RGBA32UI"][components - 1],
        format: `${channels}_INTEGER`,
        type: "UNSIGNED_INT",
        arrayType: "Uint32Array",
      };
    case "unorm8":
      return {
        internalFormat: ["R8", "RG8", "RGB8", "RGBA8"][components - 1],
        format: channels,
        type: "UNSIGNED_BYTE",
        arrayType: "Uint8Array",
      };
  }
  throw new Error(`unknown scalar type ${scalarType}`);
}

/** Copies one overlay buffer out of wasm memory into a texture bound to its layout's uniform. */
function overlayTexture(view) {
  const format = textureFormat(view.layout);
  const data = new globalThis[format.arrayType](
    memory.buffer,
    view.address,
    view.elementCount * view.layout.components,
  );

  return {
    uniform: view.layout.uniformName,
    ...format,
    width: view.elementCount,
    height: 1,
    data: Array.from(data),
  };
}

const IDENTITY = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

/** Default hook uniforms: identity transforms, so model positions are clip positions. */
function defaultHookUniforms() {
  return {
    [uniforms.view.modelView]: { type: "mat4", value: IDENTITY },
    [uniforms.view.projection]: { type: "mat4", value: IDENTITY },
    [uniforms.view.viewportSize]: { type: "vec2", value: [CANVAS_SIZE, CANVAS_SIZE] },
    [uniforms.depthBias]: { type: "float", value: 0 },
  };
}

/** `OverlayViews.componentKeys` field by `ComponentType`. */
const KEY_VIEWS = {
  [ComponentType.Vertex]: "verts",
  [ComponentType.Edge]: "edges",
  [ComponentType.Face]: "faces",
};

/** Copies one component type's keys out of wasm memory. */
function readKeys(view) {
  return Array.from(new BigUint64Array(memory.buffer, view.address, view.elementCount));
}

/** Vertices at clip (-0.5, -0.5), (0.5, -0.5), and (0, 0.5) with identity transforms. */
const TRIANGLE = [-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0];

/**
 * Builds a one-triangle mesh in an editor, optionally selects one component, and returns the
 * mesh's overlay views. Free the editor once the views are consumed.
 */
function triangleOverlay(positions, selection) {
  const editor = new MeshEditor();
  const key = editor.addMesh(
    Mesh.fromBuffers(new Float32Array(positions), new Uint32Array([0, 1, 2])),
  );

  if (selection) {
    const { componentType, index } = selection;
    const keys = readKeys(editor.overlayViews(key).componentKeys[KEY_VIEWS[componentType]]);

    editor.setMode(1 << componentType);
    editor.setMeshActive(key);
    editor.setActiveTool(SelectTool.addTo(editor));
    editor.pointerClick({
      pick: [
        { mesh: key, componentKeys: { [KEY_VIEWS[componentType]]: BigUint64Array.of(keys[index]) } },
      ],
    });
  }

  const result = new UpdateResult();
  editor.update(0, result);
  result.free();

  return { editor, views: editor.overlayViews(key) };
}

/** Reads the RGBA pixel at `(x, y)` pixels from the bottom-left corner. */
function pixelAt(pixels, x, y) {
  const offset = (y * CANVAS_SIZE + x) * 4;
  return pixels.slice(offset, offset + 4);
}

for (const [name, expected] of Object.entries(activeUniforms)) {
  test(`${name} shader compiles and links with the default hooks`, async () => {
    const result = await page.evaluate((sources) => {
      const { program, ...rest } = window.buildShader(sources);
      return rest;
    }, withDefaultHooks(name));

    assert.equal(result.error, undefined);
    assert.ok(
      result.linked,
      [
        `${name} failed to link`,
        `vertex: ${result.vertexLog}`,
        `fragment: ${result.fragmentLog}`,
        `link: ${result.linkLog}`,
      ].join("\n"),
    );
    assert.deepEqual([...result.uniforms].sort(), [...expected].sort());
  });
}

test("point shader draws a disc per vertex, colored by selection", async () => {
  const { editor, views } = triangleOverlay(TRIANGLE, {
    componentType: ComponentType.Vertex,
    index: 1,
  });
  const textures = [overlayTexture(views.positions), overlayTexture(views.flags.verts)];
  editor.free();

  const { error, pixels } = await page.evaluate((args) => window.renderShader(args), {
    sources: withDefaultHooks("point"),
    textures,
    uniforms: {
      ...defaultHookUniforms(),
      [uniforms.point.size]: { type: "float", value: 12 },
      [uniforms.point.color]: { type: "vec4", value: [1, 0, 0, 1] },
      [uniforms.point.selectedColor]: { type: "vec4", value: [0, 1, 0, 1] },
    },
    mode: "TRIANGLE_STRIP",
    vertexCount: 4,
    instanceCount: views.positions.elementCount,
  });
  assert.equal(error, undefined);

  const red = [255, 0, 0, 255];
  const green = [0, 255, 0, 255];
  const black = [0, 0, 0, 255];

  // Vertices at clip (-0.5, -0.5), (0.5, -0.5), and (0, 0.5) land on pixels (16, 16), (48, 16),
  // and (32, 48). Vertex 1 is selected. Discs have a 6 pixel radius.
  assert.deepEqual(pixelAt(pixels, 16, 16), red);
  assert.deepEqual(pixelAt(pixels, 48, 16), green);
  assert.deepEqual(pixelAt(pixels, 32, 48), red);
  assert.deepEqual(pixelAt(pixels, 20, 16), red, "inside the radius");
  assert.deepEqual(pixelAt(pixels, 24, 16), black, "outside the radius");
  assert.deepEqual(pixelAt(pixels, 18, 18), red, "inside the disc, off-axis");
  assert.deepEqual(pixelAt(pixels, 21, 21), black, "inside the quad's corner, outside the disc");
  assert.deepEqual(pixelAt(pixels, 32, 16), black, "between points");
});

/** Reads one overlay buffer's elements out of wasm memory, one array per element. */
function readElements(view) {
  const { components } = view.layout;
  const data = new Uint32Array(memory.buffer, view.address, view.elementCount * components);
  return Array.from({ length: view.elementCount }, (_, i) =>
    Array.from(data.subarray(i * components, (i + 1) * components)),
  );
}

function edgeUniforms() {
  return {
    [uniforms.edge.width]: { type: "float", value: 4 },
    [uniforms.edge.color]: { type: "vec4", value: [1, 0, 0, 1] },
    [uniforms.edge.selectedColor]: { type: "vec4", value: [0, 1, 0, 1] },
  };
}

test("edge shader draws a fixed-width quad per edge, colored by selection", async () => {
  const { editor, views } = triangleOverlay(TRIANGLE, {
    componentType: ComponentType.Edge,
    index: 0,
  });
  assert.deepEqual(readElements(views.edges), [
    [0, 1],
    [1, 2],
    [0, 2],
  ]);
  const textures = [
    overlayTexture(views.positions),
    overlayTexture(views.flags.edges),
    overlayTexture(views.edges),
  ];
  editor.free();

  const { error, pixels } = await page.evaluate((args) => window.renderShader(args), {
    sources: withDefaultHooks("edge"),
    textures,
    uniforms: { ...defaultHookUniforms(), ...edgeUniforms() },
    mode: "TRIANGLE_STRIP",
    vertexCount: 4,
    instanceCount: views.edges.elementCount,
  });
  assert.equal(error, undefined);

  const red = [255, 0, 0, 255];
  const green = [0, 255, 0, 255];
  const black = [0, 0, 0, 255];

  // Edge 0 (selected) runs along pixel row 16 from x 16 to 48. Edges are 4 pixels wide.
  assert.deepEqual(pixelAt(pixels, 32, 16), green);
  assert.deepEqual(pixelAt(pixels, 32, 15), green);
  assert.deepEqual(pixelAt(pixels, 32, 19), black, "above the half-width");
  assert.deepEqual(pixelAt(pixels, 32, 13), black, "below the half-width");
  // Midpoints of edges 1 and 2.
  assert.deepEqual(pixelAt(pixels, 40, 32), red);
  assert.deepEqual(pixelAt(pixels, 24, 32), red);
  assert.deepEqual(pixelAt(pixels, 32, 32), black, "inside the triangle");
});

for (const [label, bZ] of [
  ["behind the camera", 2],
  ["in the camera plane (w = 0)", 0],
]) {
  test(`edge shader draws edges with an endpoint ${label}`, async () => {
    // Perspective camera at the origin looking down -z: 90 degree field of view, near 0.1, far 100.
    const near = 0.1;
    const far = 100;
    // prettier-ignore
    const perspective = [
      1, 0, 0, 0,
      0, 1, 0, 0,
      0, 0, (far + near) / (near - far), -1,
      0, 0, (2 * far * near) / (near - far), 0,
    ];

    // A triangle in the plane x = 0, edge-on to the camera. A and C are in front of it and B
    // isn't, so edges A-B and B-C leave the view. A projects to pixel (32, 28.8) and C to
    // (32, 16). Approaching the camera plane, the edges' projections run off the bottom.
    const { editor, views } = triangleOverlay([0, -0.2, -2, 0, -0.2, bZ, 0, -1, -2]);
    const textures = [
      overlayTexture(views.positions),
      overlayTexture(views.flags.edges),
      overlayTexture(views.edges),
    ];
    editor.free();

    const { error, pixels } = await page.evaluate((args) => window.renderShader(args), {
      sources: withDefaultHooks("edge"),
      textures,
      uniforms: {
        ...defaultHookUniforms(),
        ...edgeUniforms(),
        [uniforms.view.projection]: { type: "mat4", value: perspective },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.edges.elementCount,
    });
    assert.equal(error, undefined);

    const red = [255, 0, 0, 255];
    const black = [0, 0, 0, 255];

    // The line is centered on x = 32 and 4 pixels wide at every height, down to the bottom.
    for (const y of [1, 10, 22]) {
      assert.deepEqual(pixelAt(pixels, 31, y), red, `(31, ${y})`);
      assert.deepEqual(pixelAt(pixels, 32, y), red, `(32, ${y})`);
      assert.deepEqual(pixelAt(pixels, 29, y), black, `(29, ${y}) is past the half-width`);
      assert.deepEqual(pixelAt(pixels, 34, y), black, `(34, ${y}) is past the half-width`);
    }
    for (const [x, y] of [
      [32, 34],
      [32, 50],
      [32, 62],
      [10, 10],
      [54, 10],
    ]) {
      assert.deepEqual(pixelAt(pixels, x, y), black, `(${x}, ${y}) should be empty`);
    }
  });
}

/**
 * Hooks like a consumer's pick pass: `setVertexOutputs` passes the component index to a varying,
 * and `writeColor` encodes it as red = index + 1.
 */
function withPickHooks(name) {
  const sources = shaders[name];
  const emptyOutputs = /void topologyOverlay_setVertexOutputs\([^)]*\)\s*\{\s*\}/;
  assert.match(shaders.defaultHooks.vertex, emptyOutputs);

  const vertexHooks = shaders.defaultHooks.vertex.replace(
    emptyOutputs,
    `flat out uint v_pickIndex;

void topologyOverlay_setVertexOutputs(vec4 positionEC, uint componentIndex)
{
    v_pickIndex = componentIndex;
}`,
  );
  const fragmentHooks = `flat in uint v_pickIndex;
out vec4 out_FragColor;

void topologyOverlay_writeColor(vec4 color)
{
    out_FragColor = vec4(float(v_pickIndex + 1u) / 255.0, 0.0, 0.0, 1.0);
}`;

  return {
    vertex: `#version 300 es\n${sources.vertex}\n${vertexHooks}`,
    fragment: `#version 300 es\n${sources.fragment}\n${fragmentHooks}`,
  };
}

/**
 * Two triangles sharing an edge, so faces 0 and 1 are distinguishable:
 * (-0.5, -0.5), (0.5, -0.5), (0, 0.5), and (0.5, 0.5). Face 0 covers pixel (32, 24); face 1
 * covers (44, 36).
 */
function twoTriangleEditor() {
  const editor = new MeshEditor();
  const key = editor.addMesh(
    Mesh.fromBuffers(
      new Float32Array([-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0, 0.5, 0.5, 0]),
      new Uint32Array([0, 1, 2, 1, 3, 2]),
    ),
  );
  const result = new UpdateResult();
  editor.update(0, result);
  result.free();

  return { editor, key };
}

function facePickDraw(views) {
  return {
    textures: [
      overlayTexture(views.positions),
      overlayTexture(views.flags.faces),
      overlayTexture(views.triangles),
    ],
    uniforms: { [uniforms.face.isPickPass]: { type: "bool", value: true } },
    mode: "TRIANGLES",
    vertexCount: 3,
    instanceCount: views.triangles.elementCount,
  };
}

/** Renders `draw` with the pick hooks and returns a reader of the overlay index at a pixel. */
async function renderPick(name, draw) {
  const { error, pixels } = await page.evaluate((args) => window.renderShader(args), {
    sources: withPickHooks(name),
    ...draw,
    uniforms: { ...defaultHookUniforms(), ...draw.uniforms },
  });
  assert.equal(error, undefined);
  return (x, y) => pixelAt(pixels, x, y)[0] - 1;
}

test("setVertexOutputs receives each drawn component's overlay index", async () => {
  const { editor, key } = twoTriangleEditor();
  const views = editor.overlayViews(key);
  const positions = overlayTexture(views.positions);
  const edges = readElements(views.edges);
  const draws = {
    point: {
      textures: [positions, overlayTexture(views.flags.verts)],
      uniforms: {
        [uniforms.point.size]: { type: "float", value: 6 },
        [uniforms.point.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.positions.elementCount,
    },
    edge: {
      textures: [positions, overlayTexture(views.flags.edges), overlayTexture(views.edges)],
      uniforms: {
        [uniforms.edge.width]: { type: "float", value: 4 },
        [uniforms.edge.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.edges.elementCount,
    },
    face: facePickDraw(views),
  };
  editor.free();

  const render = (name) => renderPick(name, draws[name]);

  // Vertices land on pixels (16, 16), (48, 16), (32, 48), and (48, 48).
  const point = await render("point");
  assert.deepEqual([point(16, 16), point(48, 16), point(32, 48), point(48, 48)], [0, 1, 2, 3]);

  // Each edge's midpoint, in pixels, maps to its index in the edge buffer.
  const pixel = ([x, y]) => [Math.floor((x + 1) * 32), Math.floor((y + 1) * 32)];
  const vertex = (i) => [-0.5, -0.5, 0.5, -0.5, 0, 0.5, 0.5, 0.5].slice(i * 2, i * 2 + 2);
  const edge = await render("edge");
  edges.forEach(([a, b], index) => {
    const [ax, ay] = vertex(a);
    const [bx, by] = vertex(b);
    assert.equal(edge(...pixel([(ax + bx) / 2, (ay + by) / 2])), index, `edge ${a}-${b}`);
  });

  // Uncovered pixels read as -1.
  const face = await render("face");
  assert.deepEqual([face(32, 24), face(44, 36), face(8, 56)], [0, 1, -1]);
});

/** `ComponentFlags.HIDDEN`, as documented on `OverlayFlagViews`. */
const HIDDEN = 2;

const ALL_COMPONENTS =
  (1 << ComponentType.Vertex) | (1 << ComponentType.Edge) | (1 << ComponentType.Face);

/** Activates the mesh with every component type interactable. */
function activate(editor, key) {
  editor.setMode(ALL_COMPONENTS);
  editor.setMeshActive(key);
}

/** Runs an update, so overlay buffers reflect the latest mesh changes. */
function update(editor) {
  const result = new UpdateResult();
  editor.update(0, result);
  result.free();
}

/** Indices of the hidden components in one flag buffer. */
function hiddenIndices(view) {
  const flags = new Uint8Array(memory.buffer, view.address, view.elementCount);
  return [...flags.keys()].filter((index) => flags[index] & HIDDEN);
}

/** Overlay indices of the hidden components of each type. */
function hiddenByType(editor, key) {
  const { flags } = editor.overlayViews(key);
  return {
    verts: hiddenIndices(flags.verts),
    edges: hiddenIndices(flags.edges),
    faces: hiddenIndices(flags.faces),
  };
}

/** Copies of the keys at `indices` in one of `componentKeys`' views. */
function keysAt(view, indices) {
  const keys = readKeys(view);
  return BigUint64Array.from(indices, (index) => keys[index]);
}

test("hideComponents hides components so they aren't drawn or picked", async () => {
  const { editor, key } = twoTriangleEditor();
  activate(editor, key);
  const { componentKeys } = editor.overlayViews(key);
  const edges = readElements(editor.overlayViews(key).edges);
  const sharedEdge = edges.findIndex(([a, b]) => a + b === 3);

  assert.equal(
    editor.hideComponents(key, {
      verts: keysAt(componentKeys.verts, [1]),
      edges: keysAt(componentKeys.edges, [sharedEdge]),
      faces: keysAt(componentKeys.faces, [1]),
    }),
    true,
  );
  update(editor);

  const views = editor.overlayViews(key);
  const positions = overlayTexture(views.positions);
  const draws = {
    point: {
      textures: [positions, overlayTexture(views.flags.verts)],
      uniforms: {
        [uniforms.point.size]: { type: "float", value: 6 },
        [uniforms.point.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.positions.elementCount,
    },
    edge: {
      textures: [positions, overlayTexture(views.flags.edges), overlayTexture(views.edges)],
      uniforms: {
        [uniforms.edge.width]: { type: "float", value: 4 },
        [uniforms.edge.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.edges.elementCount,
    },
    face: facePickDraw(views),
  };
  editor.free();

  const render = (name) => renderPick(name, draws[name]);

  // Vertex 1 is at pixel (48, 16).
  const point = await render("point");
  assert.deepEqual([point(16, 16), point(48, 16), point(32, 48), point(48, 48)], [0, -1, 2, 3]);

  // The shared edge, from vertex 1 to 2, has its midpoint at pixel (40, 32).
  const edge = await render("edge");
  assert.equal(edge(40, 32), -1);
  assert.equal(edge(32, 16), edges.findIndex(([a, b]) => a + b === 1));

  const face = await render("face");
  assert.deepEqual([face(32, 24), face(44, 36)], [0, -1]);
});

test("hideComponents extends to connected faces, and showComponents reverses it", () => {
  const { editor, key } = twoTriangleEditor();
  activate(editor, key);
  const { componentKeys } = editor.overlayViews(key);
  const verts = keysAt(componentKeys.verts, [0]);

  editor.hideComponents(key, { verts }, { extendToConnectedFaces: true });
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [0], edges: [], faces: [0] });

  editor.showComponents(key, { verts });
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [0] });

  editor.hideComponents(key, { verts });
  editor.showComponents(key, { verts }, { extendToConnectedFaces: true });
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });
  editor.free();
});

test("hideComponents ignores non-interactable types and invalid keys", () => {
  const { editor, key } = twoTriangleEditor();
  editor.setMode(1 << ComponentType.Vertex);
  editor.setMeshActive(key);
  const { componentKeys } = editor.overlayViews(key);

  editor.hideComponents(key, {
    verts: BigUint64Array.of(0n, ...keysAt(componentKeys.verts, [2])),
    faces: keysAt(componentKeys.faces, [0]),
  });
  update(editor);

  assert.deepEqual(hiddenByType(editor, key), { verts: [2], edges: [], faces: [] });
  editor.free();
});

test("showAllComponents shows every hidden component", () => {
  const { editor, key } = twoTriangleEditor();
  activate(editor, key);
  const { componentKeys } = editor.overlayViews(key);
  editor.hideComponents(key, {
    verts: keysAt(componentKeys.verts, [0, 3]),
    edges: keysAt(componentKeys.edges, [1]),
    faces: keysAt(componentKeys.faces, [0, 1]),
  });
  editor.setMode(0);

  assert.equal(editor.showAllComponents(key), true);
  update(editor);

  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });
  editor.free();
});

test("hiding needs an active mesh and throws for unknown meshes", () => {
  const { editor, key } = twoTriangleEditor();
  const verts = keysAt(editor.overlayViews(key).componentKeys.verts, [0]);

  assert.equal(editor.hideComponents(key, { verts }), false);
  assert.equal(editor.showComponents(key, { verts }), false);
  assert.equal(editor.showAllComponents(key), false);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });

  editor.removeMesh(key).free();
  for (const call of [
    () => editor.hideComponents(key, { verts }),
    () => editor.showComponents(key, { verts }),
    () => editor.showAllComponents(key),
  ]) {
    assert.throws(call, /is not registered/);
  }
  editor.free();
});

test("a picked overlay index selects its component through componentKeys", async () => {
  const { editor, key } = twoTriangleEditor();
  const views = editor.overlayViews(key);
  const faceKeys = readKeys(views.componentKeys.faces);

  const index = (await renderPick("face", facePickDraw(views)))(44, 36);
  assert.equal(index, 1);

  editor.setMode(1 << ComponentType.Face);
  editor.setMeshActive(key);
  editor.setActiveTool(SelectTool.addTo(editor));
  editor.pointerClick({
    pick: [{ mesh: key, componentKeys: { faces: BigUint64Array.of(faceKeys[index]) } }],
  });
  const result = new UpdateResult();
  editor.update(0, result);
  result.free();

  const { address, elementCount } = editor.overlayViews(key).flags.faces;
  assert.deepEqual(Array.from(new Uint8Array(memory.buffer, address, elementCount)), [0, 1]);
  editor.free();
});

/** Activates the two-triangle mesh in vertex mode. Returns a getter for its selected vertices. */
function vertexSelection(editor, key) {
  editor.setMode(1 << ComponentType.Vertex);
  editor.setMeshActive(key);

  return () => {
    update(editor);
    return selectedByType(editor, key).verts;
  };
}

test("selection edits set, add, remove, toggle, and clear components", () => {
  const { editor, key } = twoTriangleEditor();
  const selectedVerts = vertexSelection(editor, key);
  const verts = (...indices) => keysAt(editor.overlayViews(key).componentKeys.verts, indices);

  assert.equal(editor.setSelection(key, { verts: verts(0, 1) }), true);
  assert.deepEqual(selectedVerts(), [0, 1]);

  assert.equal(editor.selectComponents(key, { verts: verts(3) }), true);
  assert.deepEqual(selectedVerts(), [0, 1, 3]);

  assert.equal(editor.deselectComponents(key, { verts: verts(0) }), true);
  assert.deepEqual(selectedVerts(), [1, 3]);

  assert.equal(editor.toggleComponents(key, { verts: verts(1, 2) }), true);
  assert.deepEqual(selectedVerts(), [2, 3]);

  assert.equal(editor.clearSelection(key), true);
  assert.deepEqual(selectedVerts(), []);

  editor.selectComponents(key, { verts: verts(0) });
  assert.equal(editor.setSelection(key, {}), true);
  assert.deepEqual(selectedVerts(), []);
  editor.free();
});

test("selection edits ignore non-interactable types and invalid keys", () => {
  const { editor, key } = twoTriangleEditor();
  const selectedVerts = vertexSelection(editor, key);
  const { componentKeys } = editor.overlayViews(key);

  editor.selectComponents(key, {
    verts: BigUint64Array.of(0n, ...keysAt(componentKeys.verts, [2])),
    faces: keysAt(componentKeys.faces, [0]),
  });

  assert.deepEqual(selectedVerts(), [2]);
  assert.deepEqual(selectedByType(editor, key).faces, []);
  editor.free();
});

test("growSelection and shrinkSelection step across the selection boundary", () => {
  const { editor, key } = twoTriangleEditor();
  const selectedVerts = vertexSelection(editor, key);
  const { componentKeys } = editor.overlayViews(key);
  editor.setSelection(key, { verts: keysAt(componentKeys.verts, [0]) });

  assert.equal(editor.growSelection(key, SelectionStep.Edge), true);
  assert.deepEqual(selectedVerts(), [0, 1, 2]);

  assert.equal(editor.shrinkSelection(key, SelectionStep.Edge), true);
  assert.deepEqual(selectedVerts(), [0]);

  editor.growSelection(key, SelectionStep.Face);
  assert.deepEqual(selectedVerts(), [0, 1, 2]);
  editor.free();
});

test("selection edits need an active mesh and throw for unknown meshes", () => {
  const { editor, key } = twoTriangleEditor();
  const verts = keysAt(editor.overlayViews(key).componentKeys.verts, [0]);
  const calls = [
    () => editor.setSelection(key, { verts }),
    () => editor.selectComponents(key, { verts }),
    () => editor.deselectComponents(key, { verts }),
    () => editor.toggleComponents(key, { verts }),
    () => editor.clearSelection(key),
    () => editor.growSelection(key, SelectionStep.Edge),
    () => editor.shrinkSelection(key, SelectionStep.Edge),
  ];

  editor.setMode(1 << ComponentType.Vertex);
  for (const call of calls) {
    assert.equal(call(), false);
  }
  update(editor);
  assert.deepEqual(selectedByType(editor, key).verts, []);

  editor.removeMesh(key).free();
  for (const call of calls) {
    assert.throws(call, /is not registered/);
  }
  editor.free();
});

/**
 * The two-triangle editor plus a second, one-triangle mesh, both active in vertex mode with an
 * active select tool. Returns the tool, and getters for each mesh's selected vertex indices.
 */
function twoMeshSelectEditor(modifiersPolicy) {
  const { editor, key } = twoTriangleEditor();
  const other = editor.addMesh(
    Mesh.fromBuffers(new Float32Array(TRIANGLE), new Uint32Array([0, 1, 2])),
  );
  update(editor);
  editor.setMode(1 << ComponentType.Vertex);
  editor.setMeshActive(key);
  editor.setMeshActive(other);
  const tool = SelectTool.addTo(editor, modifiersPolicy);
  editor.setActiveTool(tool);

  const vertsOf = (mesh) => (...indices) => ({
    mesh,
    componentKeys: { verts: keysAt(editor.overlayViews(mesh).componentKeys.verts, indices) },
  });
  const selected = () => {
    update(editor);
    return [selectedByType(editor, key).verts, selectedByType(editor, other).verts];
  };

  return { editor, tool, first: vertsOf(key), second: vertsOf(other), selected };
}

test("a pick across meshes replaces each mesh's selection", () => {
  const { editor, first, second, selected } = twoMeshSelectEditor();

  assert.equal(editor.pointerClick({ pick: [first(0, 1), second(2)] }), true);
  assert.deepEqual(selected(), [[0, 1], [2]]);

  editor.pointerClick({ pick: [first(3)] });
  assert.deepEqual(selected(), [[3], []]);
  editor.free();
});

test("pointerClick distinguishes an omitted pick from one that found nothing", () => {
  const { editor, first, second, selected } = twoMeshSelectEditor();
  editor.pointerClick({ pick: [first(0), second(0)] });

  assert.equal(editor.pointerClick({}), false);
  assert.deepEqual(selected(), [[0], [0]]);

  assert.equal(editor.pointerClick({ pick: [] }), true);
  assert.deepEqual(selected(), [[], []]);
  editor.free();
});

test("SelectTool.addTo applies a modifiers policy", () => {
  const { editor, first, second, selected } = twoMeshSelectEditor({
    shift: SelectAction.Add,
    control: SelectAction.Remove,
  });
  editor.pointerClick({ pick: [first(0)] });

  editor.pointerClick({ pick: [first(1), second(0)], modifiers: { shift: true } });
  assert.deepEqual(selected(), [[0, 1], [0]]);

  editor.pointerClick({ pick: [first(0)], modifiers: { control: true } });
  assert.deepEqual(selected(), [[1], [0]]);
  editor.free();

  assert.throws(
    () => SelectTool.addTo(new MeshEditor(), { shift: 9 }),
    /modifiersPolicy\.shift must be a SelectAction/,
  );
});

test("SelectTool.setModifiersPolicy replaces a tool's modifiers policy", () => {
  const { editor, tool, first, selected } = twoMeshSelectEditor();
  const shiftClick = () => editor.pointerClick({ pick: [first(0)], modifiers: { shift: true } });

  SelectTool.setModifiersPolicy(editor, tool, { shift: SelectAction.Add });
  shiftClick();
  shiftClick();
  assert.deepEqual(selected()[0], [0]);

  SelectTool.setModifiersPolicy(editor, tool);
  shiftClick();
  assert.deepEqual(selected()[0], []);

  assert.throws(
    () => SelectTool.setModifiersPolicy(editor, TranslateTool.addTo(editor), {}),
    /is not a select tool/,
  );
  assert.throws(() => SelectTool.setModifiersPolicy(editor, 999, {}), /is not registered/);
  assert.throws(
    () => SelectTool.setModifiersPolicy(editor, tool, { control: 7 }),
    /modifiersPolicy\.control must be a SelectAction/,
  );
  editor.free();
});

test("the default modifiers policy toggles with Shift, Ctrl, or Cmd", () => {
  const { editor, first, selected } = twoMeshSelectEditor();

  for (const modifiers of [{ shift: true }, { control: true }, { meta: true }]) {
    editor.pointerClick({ pick: [first(0)], modifiers });
    assert.deepEqual(selected()[0], [0], JSON.stringify(modifiers));

    editor.pointerClick({ pick: [first(0)], modifiers });
    assert.deepEqual(selected()[0], [], JSON.stringify(modifiers));
  }
  editor.free();
});

/**
 * An editor with one active, vertex-mode mesh of two triangles. `_ID` is -1, 0, 1, 2 by vertex, and
 * `_COLOR` is red at vertices 1 and 2.
 */
function attributeEditor() {
  const editor = new MeshEditor();
  const key = editor.addMesh(
    Mesh.fromBuffers(
      new Float32Array([-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0, 0.5, 0.5, 0]),
      new Uint32Array([0, 1, 2, 1, 3, 2]),
      undefined,
      undefined,
      undefined,
      [
        CustomAttribute.int32("_ID", AttributeDomain.Vertex, 1, new Int32Array([-1, 0, 1, 2])),
        CustomAttribute.float32(
          "_COLOR",
          AttributeDomain.Vertex,
          3,
          new Float32Array([0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0]),
        ),
      ],
    ),
  );
  editor.setMode(1 << ComponentType.Vertex);
  editor.setMeshActive(key);

  return { editor, key };
}

/** Overlay indices of the selected vertices. */
function selectedVerts(editor, key) {
  const result = new UpdateResult();
  editor.update(0, result);
  result.free();

  const { address, elementCount } = editor.overlayViews(key).flags.verts;
  const values = new Uint8Array(memory.buffer, address, elementCount);
  return [...values.keys()].filter((index) => values[index] & 1);
}

test("selectByAttribute compares scalar attributes", () => {
  for (const [comparison, value, expected] of [
    [AttributeComparison.Equal, 0, [1]],
    [AttributeComparison.NotEqual, 0, [0, 2, 3]],
    [AttributeComparison.Less, 1, [0, 1]],
    [AttributeComparison.LessOrEqual, 1, [0, 1, 2]],
    [AttributeComparison.Greater, 0, [2, 3]],
    [AttributeComparison.GreaterOrEqual, 1, [2, 3]],
  ]) {
    const { editor, key } = attributeEditor();

    assert.equal(
      editor.selectByAttribute(key, "_ID", comparison, value),
      AttributeResult.Ok,
    );
    assert.deepEqual(selectedVerts(editor, key), expected, AttributeComparison[comparison]);
    editor.free();
  }
});

test("selectByAttribute compares vector attributes for equality only", () => {
  for (const [comparison, result, expected] of [
    [AttributeComparison.Equal, AttributeResult.Ok, [1, 2]],
    [AttributeComparison.NotEqual, AttributeResult.Ok, [0, 3]],
    [AttributeComparison.Less, AttributeResult.UnsupportedComparison, []],
    [AttributeComparison.GreaterOrEqual, AttributeResult.UnsupportedComparison, []],
  ]) {
    const { editor, key } = attributeEditor();

    assert.equal(editor.selectByAttribute(key, "_COLOR", comparison, [1, 0, 0]), result);
    assert.deepEqual(selectedVerts(editor, key), expected, AttributeComparison[comparison]);
    editor.free();
  }
});

test("selectByAttribute reports lookups that select nothing", () => {
  const { editor, key } = attributeEditor();
  const { Equal } = AttributeComparison;

  for (const [name, value, result] of [
    ["_MISSING", 0, AttributeResult.AttributeNotFound],
    ["_ID", 1.5, AttributeResult.WrongType],
    ["_COLOR", 1, AttributeResult.WrongType],
    ["_COLOR", [1, 0], AttributeResult.WrongType],
  ]) {
    assert.equal(editor.selectByAttribute(key, name, Equal, value), result, name);
  }
  assert.deepEqual(selectedVerts(editor, key), []);

  editor.setMeshInactive(key);
  assert.equal(
    editor.selectByAttribute(key, "_ID", Equal, 0),
    AttributeResult.MeshInactive,
  );

  editor.removeMesh(key).free();
  assert.throws(() => editor.selectByAttribute(key, "_ID", Equal, 0), /is not registered/);
  editor.free();
});

/** Overlay indices of the vertices with `keys`, sorted. */
function vertIndices(editor, key, keys) {
  const vertKeys = readKeys(editor.overlayViews(key).componentKeys.verts);
  return Array.from(keys, (vertKey) => vertKeys.indexOf(vertKey)).sort();
}

test("queryByAttribute returns matching keys without selecting them", () => {
  const { editor, key } = attributeEditor();
  const { Equal, GreaterOrEqual } = AttributeComparison;

  for (const [name, comparison, value, expected] of [
    ["_ID", Equal, 0, [1]],
    ["_ID", GreaterOrEqual, 0, [1, 2, 3]],
    ["_COLOR", Equal, [1, 0, 0], [1, 2]],
  ]) {
    const { status, keys } = editor.queryByAttribute(key, name, comparison, value);
    assert.equal(status, AttributeResult.Ok);
    assert.ok(keys.verts instanceof BigUint64Array);
    assert.deepEqual([keys.edges.length, keys.faces.length], [0, 0]);
    assert.deepEqual(vertIndices(editor, key, keys.verts), expected, name);
  }
  assert.deepEqual(selectedVerts(editor, key), []);

  editor.setMeshInactive(key);
  const { keys } = editor.queryByAttribute(key, "_ID", Equal, 2);
  assert.deepEqual(vertIndices(editor, key, keys.verts), [3]);

  editor.setMeshActive(key);
  editor.setSelection(key, keys);
  assert.deepEqual(selectedVerts(editor, key), [3]);
  editor.free();
});

/** `ComponentKeys` with no keys. */
const noKeys = () => ({
  verts: new BigUint64Array(),
  edges: new BigUint64Array(),
  faces: new BigUint64Array(),
});

test("componentKeys lists the requested types' keys in overlay order", () => {
  const { editor, key } = edgeAndFaceEditor(ComponentType.Vertex);
  const result = new UpdateResult();
  editor.update(0, result);
  result.free();

  const all = (1 << ComponentType.Vertex) | (1 << ComponentType.Edge) | (1 << ComponentType.Face);
  const keys = editor.componentKeys(key, all);
  const overlay = editor.overlayViews(key).componentKeys;
  assert.deepEqual([...keys.verts], readKeys(overlay.verts));
  assert.deepEqual([...keys.edges], readKeys(overlay.edges));
  assert.deepEqual([...keys.faces], readKeys(overlay.faces));
  assert.deepEqual([keys.verts.length, keys.edges.length, keys.faces.length], [4, 5, 2]);

  const edges = editor.componentKeys(key, 1 << ComponentType.Edge);
  assert.deepEqual([edges.verts.length, edges.edges.length, edges.faces.length], [0, 5, 0]);
  assert.deepEqual(editor.componentKeys(key, 0), noKeys());
  assert.throws(() => editor.componentKeys(key, 8), /invalid component mask 8/);

  const positions = new Float32Array(keys.verts.length * 3);
  editor.attributeValues(key, "POSITION", keys.verts, positions);
  assert.deepEqual([...positions.slice(0, 3)], [-0.5, -0.5, 0]);

  editor.setMeshInactive(key);
  assert.equal(editor.componentKeys(key, 1 << ComponentType.Face).faces.length, 2);
  editor.removeMesh(key).free();
  assert.throws(() => editor.componentKeys(key, all), /is not registered/);
  editor.free();
});

test("queryByAttribute reports failed lookups with no keys", () => {
  const { editor, key } = attributeEditor();
  const { Equal, Less } = AttributeComparison;

  for (const [name, comparison, value, status] of [
    ["_MISSING", Equal, 0, AttributeResult.AttributeNotFound],
    ["_ID", Equal, 1.5, AttributeResult.WrongType],
    ["_COLOR", Equal, [1, 0], AttributeResult.WrongType],
    ["_COLOR", Less, [1, 0, 0], AttributeResult.UnsupportedComparison],
  ]) {
    const result = editor.queryByAttribute(key, name, comparison, value);
    assert.deepEqual(result, { status, keys: noKeys() }, name);
  }

  editor.removeMesh(key).free();
  assert.throws(() => editor.queryByAttribute(key, "_ID", Equal, 0), /is not registered/);
  editor.free();
});

test("attributeValues reads values by key in the form selectByAttribute takes", () => {
  const { editor, key } = attributeEditor();
  const [vert0, vert1, vert2] = readKeys(editor.overlayViews(key).componentKeys.verts);

  const ids = new Int32Array(2);
  assert.equal(
    editor.attributeValues(key, "_ID", BigUint64Array.of(vert2, vert0), ids),
    AttributeResult.Ok,
  );
  assert.deepEqual([...ids], [1, -1]);

  const color = new Float32Array(3);
  editor.attributeValues(key, "_COLOR", BigUint64Array.of(vert1), color);
  assert.deepEqual([...color], [1, 0, 0]);
  editor.selectByAttribute(key, "_COLOR", AttributeComparison.Equal, color);
  assert.deepEqual(selectedVerts(editor, key), [1, 2]);

  const positions = new Float32Array(6);
  editor.attributeValues(key, "POSITION", BigUint64Array.of(vert0, vert2), positions);
  assert.deepEqual([...positions], [-0.5, -0.5, 0, 0, 0.5, 0]);

  editor.setMeshInactive(key);
  editor.attributeValues(key, "_ID", BigUint64Array.of(0n, vert1), ids, 99);
  assert.deepEqual([...ids], [99, 0]);
  editor.free();
});

test("attributeValues reports failed reads and throws for a wrong out", () => {
  const { editor, key } = attributeEditor();
  const [vert0] = readKeys(editor.overlayViews(key).componentKeys.verts);
  const keys = BigUint64Array.of(vert0);
  const { AttributeNotFound, KeyNotFound, UnsupportedDomain, WrongType } = AttributeResult;

  for (const [name, keys_, out, noData, result] of [
    ["_MISSING", keys, new Int32Array(1), undefined, AttributeNotFound],
    ["NORMAL", keys, new Float32Array(3), undefined, UnsupportedDomain],
    ["_ID", BigUint64Array.of(vert0, 0n), new Int32Array([5, 5]), undefined, KeyNotFound],
    ["_ID", BigUint64Array.of(0n), new Int32Array([5]), 1.5, WrongType],
    ["_COLOR", keys, new Float32Array([5, 5, 5]), [1, 0], WrongType],
  ]) {
    const before = [...out];
    assert.equal(editor.attributeValues(key, name, keys_, out, noData), result, name);
    assert.deepEqual([...out], before, name);
  }

  for (const [out, error] of [
    [new Float32Array(1), /out is not an Int32Array/],
    [new Int32Array(2), /out length 2 is not 1/],
  ]) {
    assert.throws(() => editor.attributeValues(key, "_ID", keys, out), error);
  }

  editor.removeMesh(key).free();
  assert.throws(
    () => editor.attributeValues(key, "_ID", keys, new Int32Array(1)),
    /is not registered/,
  );
  editor.free();
});

test("setAttributeValues writes values by key and ignores unknown keys", () => {
  const { editor, key } = attributeEditor();
  const [vert0, , vert2] = readKeys(editor.overlayViews(key).componentKeys.verts);
  const read = (name, keys, out) => (editor.attributeValues(key, name, keys, out), [...out]);

  assert.equal(
    editor.setAttributeValues(key, "_ID", BigUint64Array.of(vert2, 0n), new Int32Array([7, 8])),
    AttributeResult.Ok,
  );
  assert.deepEqual(read("_ID", BigUint64Array.of(vert0, vert2), new Int32Array(2)), [-1, 7]);

  const moved = new Float32Array([0, 0.5, 1]);
  assert.equal(
    editor.setAttributeValues(key, "POSITION", BigUint64Array.of(vert2), moved),
    AttributeResult.Ok,
  );
  assert.deepEqual(read("POSITION", BigUint64Array.of(vert2), new Float32Array(3)), [0, 0.5, 1]);

  const result = new UpdateResult();
  editor.update(0, result);
  result.free();
  const view = editor.overlayViews(key).positions;
  const overlayPositions = new Float32Array(memory.buffer, view.address, view.elementCount * 4);
  assert.deepEqual([...overlayPositions.slice(8, 11)], [0, 0.5, 1]);

  assert.throws(
    () => editor.setAttributeValues(key, "_ID", BigUint64Array.of(vert0), new Uint32Array(1)),
    /values is not an Int32Array/,
  );
  for (const [name, result] of [
    ["NORMAL", AttributeResult.UnsupportedDomain],
    ["_MISSING", AttributeResult.AttributeNotFound],
  ]) {
    const values = new Float32Array(3);
    assert.equal(editor.setAttributeValues(key, name, BigUint64Array.of(vert0), values), result);
  }

  editor.setMeshInactive(key);
  assert.equal(
    editor.setAttributeValues(key, "_ID", BigUint64Array.of(vert0), new Int32Array([5])),
    AttributeResult.MeshInactive,
  );
  assert.deepEqual(read("_ID", BigUint64Array.of(vert0), new Int32Array(1)), [-1]);
  editor.free();
});

/**
 * An editor with one active mesh of two triangles, interactable in `mode`. `_FEATURE` is 3 on the
 * first face and 4 on the second, and `_CREASE` is 1 on the shared edge and 0 on the others.
 */
function edgeAndFaceEditor(mode) {
  const editor = new MeshEditor();
  const key = editor.addMesh(
    Mesh.fromBuffers(
      new Float32Array([-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0, 0.5, 0.5, 0]),
      new Uint32Array([0, 1, 2, 1, 3, 2]),
      undefined,
      undefined,
      undefined,
      [
        CustomAttribute.uint32("_FEATURE", AttributeDomain.Face, 1, new Uint32Array([3, 4])),
        CustomAttribute.uint32(
          "_CREASE",
          AttributeDomain.Edge,
          1,
          new Uint32Array([1]),
          new Uint32Array([0]),
        ),
        CustomAttribute.float32("_CORNER", AttributeDomain.Corner, 1, new Float32Array(6)),
      ],
      new Uint32Array([2, 1]),
    ),
  );
  editor.setMode(1 << mode);
  editor.setMeshActive(key);

  return { editor, key };
}

test("addAttribute adds a writable attribute with a default value", () => {
  const { editor, key } = attributeEditor();
  const { Ok } = AttributeResult;
  const mask = (1 << ComponentType.Vertex) | (1 << ComponentType.Edge);
  const { verts, edges } = editor.componentKeys(key, mask);

  assert.equal(
    editor.addAttribute(key, "_WEIGHT", AttributeDomain.Vertex, new Float32Array([0.5])),
    Ok,
  );
  assert.equal(
    editor.addAttribute(key, "_CELL", AttributeDomain.Edge, new Int32Array([-1, 2])),
    Ok,
  );
  assert.equal(
    editor.addAttribute(key, "_SEAM", AttributeDomain.Corner, new Uint32Array([0])),
    Ok,
  );

  const weights = new Float32Array(verts.length);
  editor.attributeValues(key, "_WEIGHT", verts, weights);
  assert.deepEqual([...weights], [0.5, 0.5, 0.5, 0.5]);
  const cells = new Int32Array(edges.length * 2);
  editor.attributeValues(key, "_CELL", edges, cells);
  assert.deepEqual([...cells.slice(0, 4)], [-1, 2, -1, 2]);

  editor.setAttributeValues(key, "_WEIGHT", verts.subarray(1, 2), new Float32Array([2]));
  editor.attributeValues(key, "_WEIGHT", verts, weights);
  assert.deepEqual([...weights], [0.5, 2, 0.5, 0.5]);

  const added = editor
    .meshAttributes(key)
    .map(({ name, domain, componentCount }) => [name, domain, componentCount])
    .filter(([name]) => ["_WEIGHT", "_CELL", "_SEAM"].includes(name));
  assert.deepEqual(
    added,
    [
      ["_WEIGHT", AttributeDomain.Vertex, 1],
      ["_CELL", AttributeDomain.Edge, 2],
      ["_SEAM", AttributeDomain.Corner, 1],
    ],
  );
  editor.free();
});

test("addAttribute and removeAttribute report failures", () => {
  const { editor, key } = attributeEditor();
  const { Vertex } = AttributeDomain;
  const { AttributeNotFound, MeshInactive, NameInUse, Ok, Reserved, WrongType } = AttributeResult;

  for (const [name, defaultValue, result] of [
    ["_ID", new Int32Array([0]), NameInUse],
    ["POSITION", new Float32Array(3), Reserved],
    ["_NEW", new Float32Array(5), WrongType],
    ["_NEW", new Float32Array(0), WrongType],
    ["_NEW", new Float64Array([1]), WrongType],
    ["_NEW", [1], WrongType],
  ]) {
    assert.equal(editor.addAttribute(key, name, Vertex, defaultValue), result, name);
  }

  assert.equal(editor.removeAttribute(key, "_ID"), Ok);
  assert.equal(editor.removeAttribute(key, "_ID"), AttributeNotFound);
  assert.equal(editor.removeAttribute(key, "NORMAL"), Reserved);
  assert.equal(editor.addAttribute(key, "_ID", Vertex, new Float32Array([1])), Ok);

  editor.setMeshInactive(key);
  assert.equal(editor.addAttribute(key, "_X", Vertex, new Float32Array([1])), MeshInactive);
  assert.equal(editor.removeAttribute(key, "_ID"), MeshInactive);

  editor.removeMesh(key).free();
  assert.throws(() => editor.removeAttribute(key, "_ID"), /is not registered/);
  editor.free();
});

test("selectByAttribute and queryByAttribute operate on edges and faces", () => {
  const { Equal } = AttributeComparison;

  const faces = edgeAndFaceEditor(ComponentType.Face);
  assert.equal(faces.editor.selectByAttribute(faces.key, "_FEATURE", Equal, 4), AttributeResult.Ok);
  const faceKeys = readKeys(faces.editor.overlayViews(faces.key).componentKeys.faces);
  assert.deepEqual(faces.editor.selectedComponents(faces.key).faces, BigUint64Array.of(faceKeys[1]));
  faces.editor.free();

  const edges = edgeAndFaceEditor(ComponentType.Edge);
  const { status, keys } = edges.editor.queryByAttribute(edges.key, "_CREASE", Equal, 1);
  assert.equal(status, AttributeResult.Ok);
  const crease = new Uint32Array(1);
  edges.editor.attributeValues(edges.key, "_CREASE", keys.edges, crease);
  assert.deepEqual([keys.verts.length, keys.edges.length, crease[0]], [0, 1, 1]);
  const { keys: others } = edges.editor.queryByAttribute(edges.key, "_CREASE", Equal, 0);
  assert.equal(others.edges.length, 4);

  assert.equal(
    edges.editor.selectByAttribute(edges.key, "_CORNER", Equal, 0),
    AttributeResult.UnsupportedDomain,
  );
  assert.deepEqual(edges.editor.queryByAttribute(edges.key, "_CORNER", Equal, 0), {
    status: AttributeResult.UnsupportedDomain,
    keys: noKeys(),
  });
  edges.editor.free();
});

/** An editor like `attributeEditor`'s with integer vector attributes: `_CELL` (ivec2) is equal at
 * vertices 1 and 2, and `_MASK` (uvec4) at vertices 0 and 3. */
function integerVectorEditor() {
  const editor = new MeshEditor();
  const key = editor.addMesh(
    Mesh.fromBuffers(
      new Float32Array([-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0, 0.5, 0.5, 0]),
      new Uint32Array([0, 1, 2, 1, 3, 2]),
      undefined,
      undefined,
      undefined,
      [
        CustomAttribute.int32(
          "_CELL",
          AttributeDomain.Vertex,
          2,
          new Int32Array([0, 0, 1, -1, 1, -1, 2, 0]),
        ),
        CustomAttribute.uint32(
          "_MASK",
          AttributeDomain.Vertex,
          4,
          new Uint32Array([1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 1, 2, 3, 4]),
        ),
      ],
    ),
  );
  editor.setMode(1 << ComponentType.Vertex);
  editor.setMeshActive(key);

  return { editor, key };
}

test("integer vector attributes read back and compare for equality only", () => {
  const { Equal, NotEqual, Less } = AttributeComparison;

  for (const [name, comparison, value, result, expected] of [
    ["_CELL", Equal, [1, -1], AttributeResult.Ok, [1, 2]],
    ["_CELL", NotEqual, [1, -1], AttributeResult.Ok, [0, 3]],
    ["_MASK", Equal, new Uint32Array([1, 2, 3, 4]), AttributeResult.Ok, [0, 3]],
    ["_CELL", Less, [1, -1], AttributeResult.UnsupportedComparison, []],
    ["_CELL", Equal, [1, -1, 0], AttributeResult.WrongType, []],
    ["_CELL", Equal, [1.5, -1], AttributeResult.WrongType, []],
    ["_CELL", Equal, 1, AttributeResult.WrongType, []],
    ["_MASK", Equal, [-1, 2, 3, 4], AttributeResult.WrongType, []],
  ]) {
    const { editor, key } = integerVectorEditor();

    assert.equal(editor.selectByAttribute(key, name, comparison, value), result, name);
    assert.deepEqual(selectedVerts(editor, key), expected, `${name} ${value}`);
    editor.free();
  }

  const { editor, key } = integerVectorEditor();
  const [vert0, vert1] = readKeys(editor.overlayViews(key).componentKeys.verts);
  const cell = new Int32Array(2);
  editor.attributeValues(key, "_CELL", BigUint64Array.of(vert1), cell);
  assert.deepEqual([...cell], [1, -1]);
  const mask = new Uint32Array(4);
  editor.attributeValues(key, "_MASK", BigUint64Array.of(vert0), mask);
  assert.deepEqual([...mask], [1, 2, 3, 4]);
  editor.free();
});

test("meshAttributes lists attributes by domain with their types", () => {
  const { Vertex, Corner } = AttributeDomain;
  const { Float32, Int32 } = AttributeComponentType;
  const editor = new MeshEditor();

  const withoutUvs = editor.addMesh(
    Mesh.fromBuffers(new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]), new Uint32Array([0, 1, 2])),
  );
  assert.deepEqual(editor.meshAttributes(withoutUvs), [
    { name: "POSITION", domain: Vertex, componentType: Float32, componentCount: 3 },
    { name: "NORMAL", domain: Corner, componentType: Float32, componentCount: 3 },
  ]);

  const custom = integerVectorEditor();
  assert.deepEqual(
    custom.editor.meshAttributes(custom.key).map(({ name }) => name),
    ["POSITION", "_CELL", "_MASK", "NORMAL"],
  );
  assert.deepEqual(custom.editor.meshAttributes(custom.key)[1], {
    name: "_CELL",
    domain: Vertex,
    componentType: Int32,
    componentCount: 2,
  });
  custom.editor.free();

  const withUvs = editor.addMesh(
    Mesh.fromBuffers(
      new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
      new Uint32Array([0, 1, 2]),
      undefined,
      new Float32Array([0, 0, 1, 0, 0, 1]),
      undefined,
      [CustomAttribute.float32("_COLOR", Corner, 4, new Float32Array(12))],
    ),
  );
  assert.deepEqual(editor.meshAttributes(withUvs).slice(2), [
    { name: "TEXCOORD_0", domain: Corner, componentType: Float32, componentCount: 2 },
    { name: "_COLOR", domain: Corner, componentType: Float32, componentCount: 4 },
  ]);

  editor.free();
});

test("CustomAttribute constructors throw for a componentCount that doesn't fit the values", () => {
  const { Vertex } = AttributeDomain;

  for (const create of [
    () => CustomAttribute.uint32("_A", Vertex, 0, new Uint32Array(4)),
    () => CustomAttribute.int32("_A", Vertex, 5, new Int32Array(5)),
    () => CustomAttribute.float32("_A", Vertex, 0, new Float32Array(4)),
  ]) {
    assert.throws(create, /_A componentCount \d is not 1, 2, 3 or 4/);
  }

  for (const create of [
    () => CustomAttribute.uint32("_A", Vertex, 3, new Uint32Array(4)),
    () => CustomAttribute.int32("_A", Vertex, 2, new Int32Array(3)),
  ]) {
    assert.throws(create, /_A length \d is not a multiple of \d/);
  }
});

test("hideByAttribute and showByAttribute hide and show matching vertices", () => {
  const { editor, key } = attributeEditor();
  const { Equal, Less, GreaterOrEqual } = AttributeComparison;
  const { Ok } = AttributeResult;
  const extend = { extendToConnectedFaces: true };

  assert.equal(editor.hideByAttribute(key, "_ID", Less, 0), Ok);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [0], edges: [], faces: [] });

  // Vertex 3 is only in face 1.
  assert.equal(editor.hideByAttribute(key, "_ID", Equal, 2, extend), Ok);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [0, 3], edges: [], faces: [1] });

  assert.equal(editor.showByAttribute(key, "_ID", GreaterOrEqual, 0), Ok);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [0], edges: [], faces: [1] });

  // Vertex 0 is only in face 0, so face 1 stays hidden.
  assert.equal(editor.showByAttribute(key, "_ID", Less, 0, extend), Ok);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [1] });

  assert.equal(editor.showByAttribute(key, "_ID", Equal, 2, extend), Ok);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });
  editor.free();
});

test("hideByAttribute reports lookups that hide nothing", () => {
  const { editor, key } = attributeEditor();
  const { Equal, Less } = AttributeComparison;

  for (const [name, comparison, value, result] of [
    ["_MISSING", Equal, 0, AttributeResult.AttributeNotFound],
    ["_ID", Equal, 1.5, AttributeResult.WrongType],
    ["_COLOR", Less, [1, 0, 0], AttributeResult.UnsupportedComparison],
  ]) {
    assert.equal(editor.hideByAttribute(key, name, comparison, value), result, name);
  }

  editor.setMode(1 << ComponentType.Face);
  assert.equal(
    editor.hideByAttribute(key, "_ID", Equal, 0),
    AttributeResult.Ok,
    "vertices aren't interactable",
  );
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });

  editor.setMeshInactive(key);
  for (const call of [
    () => editor.hideByAttribute(key, "_ID", Equal, 0),
    () => editor.showByAttribute(key, "_ID", Equal, 0),
  ]) {
    assert.equal(call(), AttributeResult.MeshInactive);
  }

  editor.removeMesh(key).free();
  assert.throws(() => editor.hideByAttribute(key, "_ID", Equal, 0), /is not registered/);
  editor.free();
});

/** Overlay indices of the selected components of each type. */
function selectedByType(editor, key) {
  const selected = (view) => {
    const flags = new Uint8Array(memory.buffer, view.address, view.elementCount);
    return [...flags.keys()].filter((index) => flags[index] & 1);
  };
  const { flags } = editor.overlayViews(key);

  return { verts: selected(flags.verts), edges: selected(flags.edges), faces: selected(flags.faces) };
}

/**
 * Activates the two-triangle mesh in edge mode and selects the edge from vertex 0 to 1, which is
 * only in face 0. Returns that edge's overlay index.
 */
function selectOuterEdge(editor, key) {
  const edge = readElements(editor.overlayViews(key).edges).findIndex(([a, b]) => a + b === 1);
  const [componentKey] = keysAt(editor.overlayViews(key).componentKeys.edges, [edge]);

  editor.setMode(1 << ComponentType.Edge);
  editor.setMeshActive(key);
  editor.setActiveTool(SelectTool.addTo(editor));
  editor.pointerClick({ pick: { mesh: key, componentType: ComponentType.Edge, componentKey } });
  update(editor);

  return edge;
}

test("hideSelected deselects and hides the selected components of interactable types", () => {
  const { editor, key } = twoTriangleEditor();
  const edge = selectOuterEdge(editor, key);
  assert.deepEqual(selectedByType(editor, key), { verts: [0, 1], edges: [edge], faces: [] });

  assert.equal(editor.hideSelected(key), true);
  update(editor);

  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [edge], faces: [] });
  assert.deepEqual(selectedByType(editor, key), { verts: [], edges: [], faces: [] });
  editor.free();
});

test("hideSelected extends to connected faces", () => {
  const { editor, key } = twoTriangleEditor();
  const edge = selectOuterEdge(editor, key);

  editor.hideSelected(key, { extendToConnectedFaces: true });
  update(editor);

  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [edge], faces: [0] });
  editor.free();
});

test("hideSelected needs an active mesh and throws for unknown meshes", () => {
  const { editor, key } = twoTriangleEditor();
  const edge = selectOuterEdge(editor, key);
  editor.setMeshInactive(key);

  assert.equal(editor.hideSelected(key), false);
  update(editor);
  assert.deepEqual(hiddenByType(editor, key), { verts: [], edges: [], faces: [] });
  assert.deepEqual(selectedByType(editor, key), { verts: [0, 1], edges: [edge], faces: [] });

  editor.removeMesh(key).free();
  assert.throws(() => editor.hideSelected(key), /is not registered/);
  editor.free();
});
