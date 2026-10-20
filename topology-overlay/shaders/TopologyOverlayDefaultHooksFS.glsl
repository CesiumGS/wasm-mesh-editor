// Topology overlay: default fragment hooks for plain WebGL2 consumers.

out vec4 out_FragColor;

void topologyOverlay_writeColor(vec4 color)
{
    out_FragColor = color;
}
