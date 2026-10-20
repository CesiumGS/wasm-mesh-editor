use std::collections::BTreeSet;

use super::*;

const SHADERS: [(&str, ShaderSources); 3] = [("point", POINT), ("edge", EDGE), ("face", FACE)];

/// Names of `topologyOverlay_*` functions declared without a body (`...);`).
fn hook_prototypes(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(");") && !line.starts_with("//"))
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|call| call.split('(').next())
        .filter(|name| name.starts_with("topologyOverlay_"))
        .collect()
}

fn defines(source: &str, function: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        !line.ends_with(';')
            && line
                .split_whitespace()
                .nth(1)
                .is_some_and(|call| call.split('(').next() == Some(function))
    })
}

#[test]
fn shaders_declare_the_hook_interface() {
    for (name, shader) in SHADERS {
        assert_eq!(
            hook_prototypes(shader.vertex),
            [
                "topologyOverlay_modelToEye",
                "topologyOverlay_eyeToClip",
                "topologyOverlay_viewportSize",
                "topologyOverlay_setVertexOutputs",
            ],
            "{name}"
        );
        assert_eq!(
            hook_prototypes(shader.fragment),
            ["topologyOverlay_writeColor"],
            "{name}"
        );
    }
}

#[test]
fn default_hooks_define_every_declared_hook() {
    for (_, shader) in SHADERS {
        for (stage, source, hooks) in [
            ("vertex", shader.vertex, DEFAULT_HOOKS.vertex),
            ("fragment", shader.fragment, DEFAULT_HOOKS.fragment),
        ] {
            for hook in hook_prototypes(source) {
                assert!(
                    defines(hooks, hook),
                    "default {stage} hooks don't define {hook}"
                );
            }
        }
    }
}

#[test]
fn shaders_start_with_their_stage_declarations() {
    for (name, shader) in SHADERS {
        assert!(
            shader
                .vertex
                .starts_with("// Topology overlay: declarations shared by every vertex"),
            "{name}"
        );
        assert!(
            shader
                .fragment
                .starts_with("// Topology overlay: declarations shared by every fragment"),
            "{name}"
        );
        assert_eq!(shader.vertex.matches("void main()").count(), 1, "{name}");
        assert_eq!(shader.fragment.matches("void main()").count(), 1, "{name}");
    }
}

/// Names of the uniforms that `sources` declare, e.g. `uniform highp sampler2D name;`.
fn declared_uniforms(sources: ShaderSources) -> BTreeSet<&'static str> {
    [sources.vertex, sources.fragment]
        .into_iter()
        .flat_map(str::lines)
        .map(str::trim)
        .filter(|line| line.starts_with("uniform "))
        .filter_map(|line| line.strip_suffix(';')?.split_whitespace().last())
        .collect()
}

#[test]
fn shaders_declare_exactly_their_exported_uniforms() {
    use uniforms::*;

    let shared = [POSITION_TEXTURE, SELECTION_TEXTURE, DEPTH_BIAS];

    for (name, shader, specific) in [
        (
            "point",
            POINT,
            &[POINT_SIZE, POINT_COLOR, POINT_SELECTED_COLOR][..],
        ),
        (
            "edge",
            EDGE,
            &[EDGE_TEXTURE, EDGE_WIDTH, EDGE_COLOR, EDGE_SELECTED_COLOR][..],
        ),
        (
            "face",
            FACE,
            &[
                TRIANGLE_TEXTURE,
                FACE_COLOR,
                FACE_SELECTED_COLOR,
                FACE_IS_PICK_PASS,
            ][..],
        ),
    ] {
        let expected: BTreeSet<_> = shared.iter().chain(specific).copied().collect();
        assert_eq!(declared_uniforms(shader), expected, "{name}");
    }

    assert_eq!(
        declared_uniforms(DEFAULT_HOOKS),
        BTreeSet::from([MODEL_VIEW, PROJECTION, VIEWPORT_SIZE])
    );
}
