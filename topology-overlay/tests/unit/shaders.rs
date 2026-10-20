use super::*;

const PROGRAMS: [(&str, ShaderSources); 3] = [("point", POINT), ("edge", EDGE), ("face", FACE)];

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
fn programs_declare_the_hook_interface() {
    for (name, program) in PROGRAMS {
        assert_eq!(
            hook_prototypes(program.vertex),
            [
                "topologyOverlay_modelToEye",
                "topologyOverlay_eyeToClip",
                "topologyOverlay_viewportSize",
                "topologyOverlay_setVertexOutputs",
            ],
            "{name}"
        );
        assert_eq!(
            hook_prototypes(program.fragment),
            ["topologyOverlay_writeColor"],
            "{name}"
        );
    }
}

#[test]
fn default_hooks_define_every_declared_hook() {
    for (_, program) in PROGRAMS {
        for (stage, source, hooks) in [
            ("vertex", program.vertex, DEFAULT_HOOKS.vertex),
            ("fragment", program.fragment, DEFAULT_HOOKS.fragment),
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
fn programs_start_with_their_stage_declarations() {
    for (name, program) in PROGRAMS {
        assert!(
            program
                .vertex
                .starts_with("// Topology overlay: declarations shared by every vertex"),
            "{name}"
        );
        assert!(
            program
                .fragment
                .starts_with("// Topology overlay: declarations shared by every fragment"),
            "{name}"
        );
        assert_eq!(program.vertex.matches("void main()").count(), 1, "{name}");
        assert_eq!(program.fragment.matches("void main()").count(), 1, "{name}");
    }
}

/// Whether `source` declares a uniform named `name`, e.g. `uniform highp sampler2D name;`.
fn declares_uniform(source: &str, name: &str) -> bool {
    source.lines().map(str::trim).any(|line| {
        line.starts_with("uniform ")
            && line
                .strip_suffix(';')
                .is_some_and(|declaration| declaration.split_whitespace().last() == Some(name))
    })
}

#[test]
fn programs_declare_their_texture_uniforms() {
    for (uniform, programs) in [
        (uniforms::POSITION_TEXTURE, &[POINT, EDGE, FACE][..]),
        (uniforms::SELECTION_TEXTURE, &[POINT, EDGE, FACE][..]),
        (uniforms::EDGE_TEXTURE, &[EDGE][..]),
        (uniforms::TRIANGLE_TEXTURE, &[FACE][..]),
    ] {
        for program in programs {
            assert!(
                declares_uniform(program.vertex, uniform),
                "{uniform} is not declared"
            );
        }
    }

    assert!(!declares_uniform(POINT.vertex, uniforms::EDGE_TEXTURE));
    assert!(!declares_uniform(POINT.vertex, uniforms::TRIANGLE_TEXTURE));
}
