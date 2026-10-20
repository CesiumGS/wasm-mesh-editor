// Topology overlay edges: one instance per edge, drawn as a screen-aligned quad of four
// vertices (TRIANGLE_STRIP) with a constant width in pixels.

// Two vertex indices per edge.
uniform highp usampler2D u_edgeTexture;
// Width in pixels.
uniform float u_edgeWidth;

// Position across the quad's width: -1 on one long side, 1 on the other.
out float v_perp;
flat out float v_selected;

// Clips the segment from `p` to `q` to the near plane, where `z + w` is 0 in clip space, moving
// an endpoint behind it to where the segment crosses it. Returns false if the whole segment is
// behind it.
//
// Extruding requires this: if an endpoint has `w <= 0`, the GPU's clipping of the extruded
// quad widens it toward that endpoint (or drops it at `w == 0`).
bool topologyOverlay_clipToNearPlane(
    inout vec4 pEC, inout vec4 pClip, inout vec4 qEC, inout vec4 qClip)
{
    float pNear = pClip.z + pClip.w;
    float qNear = qClip.z + qClip.w;

    if (pNear < 0.0 && qNear < 0.0)
    {
        return false;
    }

    // `eyeToClip` is linear, so interpolating both positions keeps them in agreement.
    if (pNear < 0.0)
    {
        float t = pNear / (pNear - qNear);
        pEC = mix(pEC, qEC, t);
        pClip = mix(pClip, qClip, t);
    }
    else if (qNear < 0.0)
    {
        float t = qNear / (qNear - pNear);
        qEC = mix(qEC, pEC, t);
        qClip = mix(qClip, pClip, t);
    }

    return true;
}

void main()
{
    uint edgeIndex = uint(gl_InstanceID);
    uvec2 endpoints = texelFetch(
        u_edgeTexture, topologyOverlay_texelCoord(edgeIndex, textureSize(u_edgeTexture, 0)), 0).xy;

    vec4 aEC = topologyOverlay_applyDepthBias(
        topologyOverlay_modelToEye(topologyOverlay_fetchPosition(endpoints.x)));
    vec4 bEC = topologyOverlay_applyDepthBias(
        topologyOverlay_modelToEye(topologyOverlay_fetchPosition(endpoints.y)));
    vec4 aClip = topologyOverlay_eyeToClip(aEC);
    vec4 bClip = topologyOverlay_eyeToClip(bEC);
    bool visible = topologyOverlay_clipToNearPlane(aEC, aClip, bEC, bClip);

    // Strip order: (A, -1), (A, 1), (B, -1), (B, 1).
    bool atB = gl_VertexID >= 2;
    float side = float(gl_VertexID & 1) * 2.0 - 1.0;
    vec4 positionEC = atB ? bEC : aEC;

    // The edge's direction on screen, from A to B at both ends so `side` means the same side
    // everywhere. This is the difference of the endpoints' normalized device coordinates,
    // scaled by `a.w * b.w > 0`, in pixel proportions.
    vec2 direction = (bClip.xy * aClip.w - aClip.xy * bClip.w) * topologyOverlay_viewportSize();
    float directionLength = length(direction);
    direction = directionLength > 0.0 ? direction / directionLength : vec2(1.0, 0.0);
    vec2 normal = vec2(-direction.y, direction.x);

    gl_Position = visible
        ? topologyOverlay_offsetByPixels(atB ? bClip : aClip, normal * side * u_edgeWidth * 0.5)
        // Entirely behind the near plane: collapse outside the clip volume.
        : vec4(2.0, 2.0, 2.0, 1.0);

    v_perp = side;
    v_selected = topologyOverlay_fetchSelected(edgeIndex);
    topologyOverlay_setVertexOutputs(positionEC, edgeIndex);
}
