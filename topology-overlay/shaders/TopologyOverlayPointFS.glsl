// Topology overlay points: an anti-aliased disc filling the quad.

in vec2 v_quadCoord;
flat in float v_selected;

uniform vec4 u_pointColor;
uniform vec4 u_pointSelectedColor;

void main()
{
    float distanceToCenter = length(v_quadCoord);
    float delta = fwidth(distanceToCenter);
    float alpha = 1.0 - smoothstep(1.0 - delta, 1.0, distanceToCenter);

    vec4 baseColor = v_selected > 0.5 ? u_pointSelectedColor : u_pointColor;
    vec4 color = vec4(baseColor.rgb, baseColor.a * alpha);
    if (color.a < 0.005) // matches 0/255 and 1/255
    {
        discard;
    }

    topologyOverlay_writeColor(color);
}
