// Topology overlay: declarations shared by every vertex shader.
//
// The overlay owns `main()`. The consumer defines the hooks below, which keeps camera
// transforms, depth writing, and pick encoding on the consumer's side.

precision highp float;
precision highp int;

// ---- Hooks (implemented by the consumer) ----

// Model space to eye space, e.g. `modelView * vec4(positionMC, 1.0)`.
vec4 topologyOverlay_modelToEye(vec3 positionMC);
// Eye space to clip space, e.g. `projection * positionEC`. Also the place to compute custom depth.
vec4 topologyOverlay_eyeToClip(vec4 positionEC);
// Viewport size in pixels. Point and edge sizes are in the same pixels.
vec2 topologyOverlay_viewportSize();
// Called once per vertex with the drawn component's overlay index, for the consumer's pick pass.
void topologyOverlay_setPickOutputs(uint componentIndex);

// ---- Overlay buffers, one texel per element ----

// `[x, y, z, 0]` per vertex.
uniform highp sampler2D u_positionTexture;
// Selection of the drawn component kind: 0 unselected, 1 selected.
uniform highp sampler2D u_selectionTexture;
// Pulls the overlay toward the camera, as a fraction of eye-space depth.
uniform float u_depthBias;

ivec2 topologyOverlay_texelCoord(uint index, ivec2 textureSize)
{
    int i = int(index);
    return ivec2(i % textureSize.x, i / textureSize.x);
}

vec3 topologyOverlay_fetchPosition(uint vertexIndex)
{
    ivec2 coord = topologyOverlay_texelCoord(vertexIndex, textureSize(u_positionTexture, 0));
    return texelFetch(u_positionTexture, coord, 0).xyz;
}

float topologyOverlay_fetchSelected(uint componentIndex)
{
    ivec2 coord = topologyOverlay_texelCoord(componentIndex, textureSize(u_selectionTexture, 0));
    return texelFetch(u_selectionTexture, coord, 0).r > 0.5 ? 1.0 : 0.0;
}

vec4 topologyOverlay_applyDepthBias(vec4 positionEC)
{
    positionEC.z += abs(positionEC.z) * u_depthBias;
    return positionEC;
}
