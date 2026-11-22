// Topology overlay edges: anti-aliased along both long sides.

in float v_perp;
flat in float v_selected;

uniform vec4 u_edgeColor;
uniform vec4 u_edgeSelectedColor;

void main()
{
    float distanceToCenter = abs(v_perp);
    float delta = fwidth(distanceToCenter);
    float alpha = 1.0 - smoothstep(1.0 - delta, 1.0, distanceToCenter);

    vec4 baseColor = v_selected > 0.5 ? u_edgeSelectedColor : u_edgeColor;
    vec4 color = vec4(baseColor.rgb, baseColor.a * alpha);
    if (color.a < 0.005) // matches 0/255 and 1/255
    {
        discard;
    }

    topologyOverlay_writeColor(color);
}
