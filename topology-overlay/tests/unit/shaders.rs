use super::*;

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
    assert_eq!(
        hook_prototypes(FACE.vertex),
        [
            "topologyOverlay_modelToEye",
            "topologyOverlay_eyeToClip",
            "topologyOverlay_viewportSize",
            "topologyOverlay_setPickOutputs",
        ]
    );
    assert_eq!(
        hook_prototypes(FACE.fragment),
        ["topologyOverlay_writeColor"]
    );
}

#[test]
fn default_hooks_define_every_declared_hook() {
    for (stage, program, hooks) in [
        ("vertex", FACE.vertex, DEFAULT_HOOKS.vertex),
        ("fragment", FACE.fragment, DEFAULT_HOOKS.fragment),
    ] {
        let declared = hook_prototypes(program);
        assert!(!declared.is_empty(), "no {stage} hooks found");

        for hook in declared {
            assert!(
                defines(hooks, hook),
                "default {stage} hooks don't define {hook}"
            );
        }
    }
}

#[test]
fn programs_start_with_their_stage_declarations() {
    assert!(
        FACE.vertex
            .starts_with("// Topology overlay: declarations shared by every vertex")
    );
    assert!(
        FACE.fragment
            .starts_with("// Topology overlay: declarations shared by every fragment")
    );
    assert_eq!(FACE.vertex.matches("void main()").count(), 1);
    assert_eq!(FACE.fragment.matches("void main()").count(), 1);
}
