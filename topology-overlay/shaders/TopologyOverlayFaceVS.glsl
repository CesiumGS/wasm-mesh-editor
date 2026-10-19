// Topology overlay faces: one instance per triangle, three vertices per instance.

// Three vertex indices and a face index per triangle.
uniform highp usampler2D u_triangleTexture;

out vec3 v_positionEC;
flat out float v_selected;

void main()
{
    ivec2 triangleCoord = topologyOverlay_texelCoord(
        uint(gl_InstanceID), textureSize(u_triangleTexture, 0));
    uvec4 triangle = texelFetch(u_triangleTexture, triangleCoord, 0);
    uint faceIndex = triangle.w;

    vec3 positionMC = topologyOverlay_fetchPosition(triangle[gl_VertexID]);
    vec4 positionEC = topologyOverlay_applyDepthBias(topologyOverlay_modelToEye(positionMC));

    v_positionEC = positionEC.xyz;
    v_selected = topologyOverlay_fetchSelected(faceIndex);
    gl_Position = topologyOverlay_eyeToClip(positionEC);
    topologyOverlay_setPickOutputs(faceIndex);
}
