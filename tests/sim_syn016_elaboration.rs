//! Public-CLI and owned-model evidence for the SYN-016 elaboration matrix.

use std::path::Path;

use llg::core::{compile, db, model};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn016_elaboration";
const EXPECTED: &str = concat!(
    "child=4 label=syn016 dim=4\n",
    "child=8 label=syn016 dim=4\n",
    "out=5/d5 bits=4/8 base=3 bump=5 twice=6\n",
);

#[test]
fn elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers() {
    let expected_stderr = "llg: $finish at time 1000 at tb:84:5\n";
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_args(
            SUITE,
            "elaboration_matrix",
            EXPECTED,
            expected_stderr,
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn edition_and_range_controls_are_single_fault_rejections() {
    sim_cli::reject_case_with_args(
        SUITE,
        "type_parameter_2001",
        "use of undeclared identifier 'type'",
        &["--edition", "2001"],
    );
    sim_cli::reject_case(
        SUITE,
        "real_extent",
        "expression type 'real' is not integral",
    );
}

#[test]
fn owned_model_preserves_specialized_parameter_types_and_generate_scopes() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("elaboration_matrix.sv");
    let output = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("SYN-016 matrix should compile");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    assert_eq!(
        database.edition(),
        compile::LanguageEdition::SystemVerilog2009
    );
    let design = model::DesignModel::from_db(&database);

    let c0 = design.instance("tb.c0").expect("c0 instance");
    let width0 = c0
        .params
        .iter()
        .find(|param| param.name == "W")
        .expect("c0 W");
    assert!(
        matches!(width0.value, Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(4))
    );
    let out0 = c0
        .ports
        .iter()
        .find(|port| port.name == "out")
        .expect("c0 out");
    assert_eq!(out0.ty.width, Some(4));
    assert!(c0.gen_scopes.iter().any(|scope| scope.name == "narrow"));

    let c1 = design.instance("tb.c1").expect("c1 instance");
    let width1 = c1
        .params
        .iter()
        .find(|param| param.name == "W")
        .expect("c1 W");
    assert!(
        matches!(width1.value, Some(llg::core::elab::Val::Bits(ref value)) if value.to_u64() == Some(8))
    );
    let out1 = c1
        .ports
        .iter()
        .find(|port| port.name == "out")
        .expect("c1 out");
    assert_eq!(out1.ty.width, Some(8));
    assert!(c1.gen_scopes.iter().any(|scope| scope.name == "wide"));
}
