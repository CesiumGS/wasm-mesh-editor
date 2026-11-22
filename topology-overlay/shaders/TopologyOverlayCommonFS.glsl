// Topology overlay: declarations shared by every fragment shader.

precision highp float;
precision highp int;

// ---- Hooks (implemented by the consumer) ----

// Writes the final color, e.g. after gamma correction, along with any log depth.
void topologyOverlay_writeColor(vec4 color);
