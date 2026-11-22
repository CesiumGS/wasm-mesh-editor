// Topology overlay: declarations shared by every vertex shader.
//
// The overlay owns `main()`. The consumer defines the hooks below, which keeps camera
// transforms, depth writing, and pick encoding on the consumer's side.

precision highp float;
precision highp int;

// ---- Hooks (implemented by the consumer) ----

// Model space to eye space, e.g. `modelView * vec4(positionMC, 1.0)`.
vec4 topologyOverlay_modelToEye(vec3 positionMC);
// Eye space to clip space, e.g. `projection * positionEC`. Must be linear, like a projection
// matrix, and free of side effects: it may be called more than once per vertex, and edges
// interpolate its results.
vec4 topologyOverlay_eyeToClip(vec4 positionEC);
// Viewport size in pixels. Point and edge sizes are in the same pixels.
vec2 topologyOverlay_viewportSize();
// Distance from the eye to the near plane, positive. The depth bias stops short of it, so a
// tight near plane doesn't clip biased points and edges. 0 disables the cap.
float topologyOverlay_nearDistance();
// Called once per vertex of each visible component, after `gl_Position` is final, with the
// vertex's eye position (before any screen-space offset) and the drawn component's overlay index. The place for per-vertex
// outputs such as custom depth and pick colors. May adjust `gl_Position`'s depth.
void topologyOverlay_setVertexOutputs(vec4 positionEC, uint componentIndex);

// ---- Overlay buffers, one texel per element ----

// `[x, y, z, 0]` per vertex.
uniform highp sampler2D u_positionTexture;
// Flags of the drawn component type: a bitwise OR of the flags below, as a normalized byte.
uniform highp sampler2D u_flagTexture;
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

const uint TOPOLOGY_OVERLAY_SELECTED = 1u;
const uint TOPOLOGY_OVERLAY_HIDDEN = 2u;

// Outside the clip volume, so the GPU discards any primitive with all its vertices here.
const vec4 TOPOLOGY_OVERLAY_CULLED = vec4(2.0, 2.0, 2.0, 1.0);

uint topologyOverlay_fetchFlags(uint componentIndex)
{
    ivec2 coord = topologyOverlay_texelCoord(componentIndex, textureSize(u_flagTexture, 0));
    // Normalized bytes convert exactly, so rounding recovers the byte.
    return uint(texelFetch(u_flagTexture, coord, 0).r * 255.0 + 0.5);
}

bool topologyOverlay_isSelected(uint flags)
{
    return (flags & TOPOLOGY_OVERLAY_SELECTED) != 0u;
}

bool topologyOverlay_isHidden(uint flags)
{
    return (flags & TOPOLOGY_OVERLAY_HIDDEN) != 0u;
}

vec4 topologyOverlay_applyDepthBias(vec4 positionEC)
{
    float biased = positionEC.z + abs(positionEC.z) * u_depthBias;
    float limit = -topologyOverlay_nearDistance() * (1.0 + 1e-5);
    positionEC.z = min(biased, max(positionEC.z, limit));
    return positionEC;
}

// Moves a clip position by a screen-space offset in pixels, keeping its depth and `w`.
vec4 topologyOverlay_offsetByPixels(vec4 positionClip, vec2 offsetPixels)
{
    // An offset of `2 / viewport` in normalized device coordinates is one pixel.
    vec2 offsetNDC = offsetPixels * 2.0 / topologyOverlay_viewportSize();
    return positionClip + vec4(offsetNDC * positionClip.w, 0.0, 0.0);
}
