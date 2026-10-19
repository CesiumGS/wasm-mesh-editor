// Topology overlay points: one instance per vertex, drawn as a screen-aligned quad of
// four vertices (TRIANGLE_STRIP).

// Diameter in pixels.
uniform float u_pointSize;

// Position across the quad: -1 to 1 on each axis.
out vec2 v_quadCoord;
flat out float v_selected;

void main()
{
    uint vertexIndex = uint(gl_InstanceID);
    vec4 positionEC = topologyOverlay_applyDepthBias(
        topologyOverlay_modelToEye(topologyOverlay_fetchPosition(vertexIndex)));

    // Strip order: (-1, -1), (1, -1), (-1, 1), (1, 1).
    vec2 corner = vec2(gl_VertexID & 1, gl_VertexID >> 1) * 2.0 - 1.0;
    gl_Position = topologyOverlay_offsetByPixels(
        topologyOverlay_eyeToClip(positionEC), corner * u_pointSize * 0.5);

    v_quadCoord = corner;
    v_selected = topologyOverlay_fetchSelected(vertexIndex);
    topologyOverlay_setVertexOutputs(positionEC, vertexIndex);
}
