//! Nothing declared in a generate block that elaboration does not
//! instantiate exists (decision S22-D10): the static locals of its tasks and
//! functions, its variables and processes in an untaken if or case branch or
//! a loop generate without iterations, and its classes with everything they
//! use. Oracles are in the fixture readme.
use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/ki_untaken_generate";

#[test]
fn untaken_subroutine_statics_and_processes_do_not_exist() {
    sim_cli::run_case(
        SUITE,
        "subroutine_statics",
        include_str!(
            "../fixtures/sim/feature_completion/ki_untaken_generate/subroutine_statics.out"
        ),
        "INFO: subroutine_statics.sv:70:5: [SIM-001] (tb @ 1 ns) $finish called\n",
        &[],
    );
}

#[test]
fn untaken_class_features_compile_and_do_not_run() {
    sim_cli::run_case(
        SUITE,
        "class_features",
        include_str!("../fixtures/sim/feature_completion/ki_untaken_generate/class_features.out"),
        "INFO: class_features.sv:75:5: [SIM-001] (tb @ 2 ns) $finish called\n",
        &[],
    );
}

/// The owned Db lists only the classes of instantiated scopes as class
/// roots; a class of an untaken generate block stays in the arena, marked
/// uninstantiated, as does the block.
#[test]
fn untaken_generate_classes_are_not_class_roots() {
    use llg::core::{
        compile,
        db::{Db, NodeKind},
    };

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/ki_untaken_generate/class_features.sv");
    let database = sim_harness::with_frontend_temp_cwd("ki-untaken-generate-db", |_| {
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))
    })
    .expect("checked compilation");
    let declared = |name: &str| -> Vec<_> {
        database
            .node_ids()
            .filter(|id| {
                matches!(database.node_kind(*id), NodeKind::ClassDef)
                    && database.node(*id).name == name
            })
            .collect()
    };
    let [base] = declared("Base")[..] else {
        panic!("one class Base")
    };
    assert!(database.classes().contains(&base));
    assert!(!database.is_uninstantiated(base));
    for name in ["E", "D", "IC", "Impl"] {
        let classes = declared(name);
        assert!(!classes.is_empty(), "class {name} is in the arena");
        for class in classes {
            assert!(
                database.is_uninstantiated(class),
                "{name} is uninstantiated"
            );
            assert!(
                !database.classes().contains(&class),
                "{name} is no class root"
            );
            assert!(
                database.class_metadata(class).is_none(),
                "{name} has no metadata"
            );
        }
    }
    assert!(
        database
            .classes()
            .iter()
            .all(|class| !database.is_uninstantiated(*class)),
        "no class root is uninstantiated"
    );
    let blocks: Vec<_> = database
        .node_ids()
        .filter(|id| database.node(*id).name == "x" && database.is_uninstantiated(*id))
        .collect();
    assert!(!blocks.is_empty(), "the untaken block x is uninstantiated");
    // The whole-arena scan of the elaborated model visits every node that
    // is not marked, in capture order.
    let instantiated: Vec<_> = database.instantiated_node_ids().collect();
    let unmarked: Vec<_> = database
        .node_ids()
        .filter(|id| !database.is_uninstantiated(*id))
        .collect();
    assert_eq!(instantiated, unmarked);
    assert!(instantiated.len() < database.node_ids().len());
    assert!(instantiated.contains(&base));
}
