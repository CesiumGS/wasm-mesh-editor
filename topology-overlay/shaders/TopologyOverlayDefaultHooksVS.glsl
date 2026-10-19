// Topology overlay: default vertex hooks for plain WebGL2 consumers.
// Pick outputs are left to consumers with a pick pass.

uniform mat4 u_modelView;
uniform mat4 u_projection;
uniform vec2 u_viewportSize;

vec4 topologyOverlay_modelToEye(vec3 positionMC)
{
    return u_modelView * vec4(positionMC, 1.0);
}

vec4 topologyOverlay_eyeToClip(vec4 positionEC)
{
    return u_projection * positionEC;
}

vec2 topologyOverlay_viewportSize()
{
    return u_viewportSize;
}

void topologyOverlay_setPickOutputs(uint componentIndex)
{
}
