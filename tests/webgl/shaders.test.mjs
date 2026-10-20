// Compiles every topology overlay shader and renders real overlay data with them in
// headless Chromium's WebGL2. Build the wasm package first:
// `wasm-pack build mesh-editor --dev --target web --out-name mesh-editor -- --features wasm`.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

import init, {
  ComponentType,
  EditMode,
  Mesh,
  MeshEditor,
  SelectTool,
  UpdateResult,
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
  "u_selectionTexture",
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
    editor.setMode(selection.mode);
    editor.setMeshActive(key);
    editor.setActiveTool(editor.addTool(SelectTool.create()));
    editor.pointerClick({
      pick: { mesh: key, componentType: selection.componentType, index: selection.index },
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
    mode: EditMode.VERTEX,
    componentType: ComponentType.Vertex,
    index: 1,
  });
  const textures = [overlayTexture(views.positions), overlayTexture(views.selection.verts)];
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
    mode: EditMode.EDGE,
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
    overlayTexture(views.selection.edges),
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
      overlayTexture(views.selection.edges),
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

test("setVertexOutputs receives each drawn component's overlay index", async () => {
  // Two triangles sharing an edge, so faces 0 and 1 are distinguishable:
  // (-0.5, -0.5), (0.5, -0.5), (0, 0.5), and (0.5, 0.5).
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
  const views = editor.overlayViews(key);
  const positions = overlayTexture(views.positions);
  const edges = readElements(views.edges);
  const draws = {
    point: {
      textures: [positions, overlayTexture(views.selection.verts)],
      uniforms: {
        [uniforms.point.size]: { type: "float", value: 6 },
        [uniforms.point.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.positions.elementCount,
    },
    edge: {
      textures: [positions, overlayTexture(views.selection.edges), overlayTexture(views.edges)],
      uniforms: {
        [uniforms.edge.width]: { type: "float", value: 4 },
        [uniforms.edge.color]: { type: "vec4", value: [1, 1, 1, 1] },
      },
      mode: "TRIANGLE_STRIP",
      vertexCount: 4,
      instanceCount: views.edges.elementCount,
    },
    face: {
      textures: [positions, overlayTexture(views.selection.faces), overlayTexture(views.triangles)],
      uniforms: { [uniforms.face.isPickPass]: { type: "bool", value: true } },
      mode: "TRIANGLES",
      vertexCount: 3,
      instanceCount: views.triangles.elementCount,
    },
  };
  editor.free();

  const render = async (name) => {
    const { error, pixels } = await page.evaluate((args) => window.renderShader(args), {
      sources: withPickHooks(name),
      ...draws[name],
      uniforms: { ...defaultHookUniforms(), ...draws[name].uniforms },
    });
    assert.equal(error, undefined);
    return (x, y) => pixelAt(pixels, x, y)[0] - 1;
  };

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

  // Face 0 covers (32, 24); face 1 covers (44, 36). Uncovered pixels read as -1.
  const face = await render("face");
  assert.deepEqual([face(32, 24), face(44, 36), face(8, 56)], [0, 1, -1]);
});
