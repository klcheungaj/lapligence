//! SYN-018 public-pipeline coverage for nested and extern module declarations.

use std::path::Path;

use llg::core::{compile, db, model};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn018_module_declarations";
const EXPECTED: &str = "extern=7 nested=13/8\n";

#[test]
fn nested_and_extern_modules_execute_in_both_compilation_unit_modes() {
    let expected_stderr = "llg: $finish at time 1000 at tb:59:5\n";
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "module_declarations",
            &["extern_child", "extern_child_body"],
            EXPECTED,
            expected_stderr,
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn extern_module_signature_and_body_failures_remain_frontend_diagnostics() {
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_mismatch",
        "extern module 'syn018_bad' does not match implementation",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_missing",
        "missing implementation for extern module 'syn018_missing'",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "extern_2001",
        "expected a declaration name",
        &["--edition", "2001"],
    );
}

#[test]
fn owned_model_keeps_nested_scope_and_extern_instance_identity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let output = compile::compile_checked(&compile::CompileOpts {
        files: [
            "extern_child.sv",
            "extern_child_body.sv",
            "module_declarations.sv",
        ]
        .into_iter()
        .map(|name| root.join(name).to_string_lossy().into_owned())
        .collect(),
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-018 module declaration matrix should compile");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);

    let extern_instance = design.instance("tb.ext").expect("extern instance");
    assert_eq!(extern_instance.def_name, "syn018_extern_child");
    let width = extern_instance
        .params
        .iter()
        .find(|param| param.name == "W")
        .expect("extern W parameter");
    assert!(matches!(
        width.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(4)
    ));

    let left = design.instance("tb.left").expect("left parent");
    let right = design.instance("tb.right").expect("right parent");
    assert!(left
        .gen_scopes
        .iter()
        .any(|scope| scope.name == "generated"));
    assert!(right
        .gen_scopes
        .iter()
        .any(|scope| scope.name == "generated"));
    let left_generated = left
        .gen_scopes
        .iter()
        .find(|scope| scope.name == "generated")
        .expect("left generated scope");
    let right_generated = right
        .gen_scopes
        .iter()
        .find(|scope| scope.name == "generated")
        .expect("right generated scope");
    let left_leaf = left_generated.children.first().expect("left nested leaf");
    let right_leaf = right_generated.children.first().expect("right nested leaf");
    assert_eq!(left_leaf.def_name, "leaf");
    assert_eq!(right_leaf.def_name, "leaf");
    assert_ne!(
        left_leaf.full_name, right_leaf.full_name,
        "same-named nested definitions must retain distinct instance paths"
    );
    assert_eq!(left_leaf.full_name, "tb.left.generated.u");
    assert_eq!(right_leaf.full_name, "tb.right.generated.u");
    let left_extra = left_leaf
        .params
        .iter()
        .find(|param| param.name == "EXTRA")
        .expect("left EXTRA parameter");
    let right_extra = right_leaf
        .params
        .iter()
        .find(|param| param.name == "EXTRA")
        .expect("right EXTRA parameter");
    assert!(matches!(
        left_extra.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(2)
    ));
    assert!(matches!(
        right_extra.value,
        Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(3)
    ));
    assert_eq!(
        left_leaf
            .ports
            .iter()
            .find(|port| port.name == "leaf_y")
            .expect("left output")
            .ty
            .width,
        Some(4)
    );
    assert_eq!(
        right_leaf
            .ports
            .iter()
            .find(|port| port.name == "leaf_y")
            .expect("right output")
            .ty
            .width,
        Some(4)
    );
}
