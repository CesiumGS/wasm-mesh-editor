// Topology overlay faces.

in vec3 v_positionEC;
flat in float v_selected;

uniform vec4 u_faceColor;
uniform vec4 u_faceSelectedColor;
uniform bool u_isPickPass;

void main()
{
    vec4 color = v_selected > 0.5 ? u_faceSelectedColor : u_faceColor;

    if (u_isPickPass)
    {
        // Even if transparent (check below), still write a color output so face remains pickable.
        topologyOverlay_writeColor(vec4(color.rgb, 1.0));
        return;
    }

    if (color.a < 0.005) // matches 0/255 and 1/255
    {
        discard;
    }

    // Flat-shaded cue: brightest facing the camera, darker at grazing angles. `abs` shades
    // back faces the same way so they don't go black.
    vec3 normalEC = normalize(cross(dFdx(v_positionEC), dFdy(v_positionEC)));
    vec3 viewEC = normalize(-v_positionEC);
    float shade = mix(0.6, 1.05, abs(dot(normalEC, viewEC)));

    topologyOverlay_writeColor(vec4(color.rgb * shade, color.a));
}
