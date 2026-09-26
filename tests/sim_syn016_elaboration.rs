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

#[test]
fn legacy_defparam_and_constant_recursion_keep_specializations_distinct() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "legacy_specialization",
            concat!(
                "legacy=f/45 signed=-1/-59 dependent=5/9 ",
                "factorial=6/6 rounded=5/5\n",
            ),
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn dependent_defaults_nominal_types_and_folded_constants_cross_unit_policies() {
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_args(
            SUITE,
            "dependent_types",
            concat!(
                "types=-2/69/-2 reset=e/7e/e sig=36/39/36 nominal=100 ",
                "masks=1/3 let=7\n",
            ),
            "",
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
        sim_cli::run_case_with_args(
            SUITE,
            "constant_pattern_keys",
            concat!(
                "keys=1 width=4 data=a0/3c/a0 tag=3 runtime=1\n",
                "keys=1 width=4 data=0a/c3/0a tag=5 runtime=1\n",
            ),
            "",
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

#[test]
fn real_file_boundaries_distinguish_unit_names_from_package_names() {
    sim_cli::run_case_with_source_prefix(
        SUITE,
        "unit_consumer",
        &["unit_provider"],
        "unit=8 bits=5 package=3\n",
        "",
        &[],
        &["--edition", "2009", "--compilation-units", "merged"],
    );
    for policy in ["separate", "merged"] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "package_consumer",
            &["unit_provider"],
            "package=5 bits=3\n",
            "",
            &[],
            &["--edition", "2009", "--compilation-units", policy],
        );
    }
}

fn compile_elaboration_fixture(fixture: &str) -> compile::CompileOut {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("compile elaboration fixture")
}

#[test]
fn equal_width_nominal_types_select_different_generate_branches_in_owned_model() {
    let output = compile_elaboration_fixture("dependent_types");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    let design = model::DesignModel::from_db(&database);
    for (path, width, signed, branch) in [
        ("tb.c0", 4, true, "original_enum"),
        ("tb.c1", 7, false, "other_type"),
        ("tb.c2", 4, true, "other_type"),
    ] {
        let instance = design.instance(path).expect("specialized instance");
        let port = instance
            .ports
            .iter()
            .find(|port| port.name == "value")
            .expect("value port");
        assert_eq!(port.ty.width, Some(width), "{path}");
        assert_eq!(port.ty.signed, signed, "{path}");
        assert!(
            instance.gen_scopes.iter().any(|scope| scope.name == branch),
            "{path}"
        );
        let unselected = if branch == "original_enum" {
            "other_type"
        } else {
            "original_enum"
        };
        assert!(
            !instance.gen_scopes.iter().any(|scope| scope.name == unselected),
            "{path}"
        );
    }
}

#[test]
fn invalid_sizes_and_incompatible_nominal_types_remain_language_errors() {
    for fixture in ["zero_extent", "negative_extent"] {
        sim_cli::reject_case_with_args(
            SUITE,
            fixture,
            "value must be positive",
            &["--edition", "2009"],
        );
    }
    sim_cli::reject_case_with_args(
        SUITE,
        "nominal_enum_mismatch",
        "no implicit conversion",
        &["--edition", "2009"],
    );
}

#[test]
fn legal_large_extent_is_a_backend_capacity_error_not_an_edition_error() {
    let output = compile_elaboration_fixture("capacity_extent");
    let database = db::Db::from_slang(&output.snapshot).expect("owned database");
    for options in [
        llg::sim::opt::OptConfig::none(),
        llg::sim::opt::OptConfig::default(),
    ] {
        let error = llg::sim::codegen::generate_from_db_with_opts(&database, &options)
            .err()
            .expect("exclusive width limit must reject before compiling C");
        let message = error.to_string();
        assert!(message.contains("1048576"), "{message}");
        assert!(message.contains("maximum") || message.contains("limit"), "{message}");
        assert!(!message.contains("strict edition profile"), "{message}");
    }
}

#[test]
fn separate_unit_visibility_and_merged_source_order_are_not_interchangeable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE);
    let provider = root.join("unit_provider.sv").to_string_lossy().into_owned();
    let consumer = root.join("unit_consumer.sv").to_string_lossy().into_owned();
    for (mode, files, accepted) in [
        (
            compile::CompilationUnitMode::Merged,
            vec![provider.clone(), consumer.clone()],
            true,
        ),
        (
            compile::CompilationUnitMode::Separate,
            vec![provider.clone(), consumer.clone()],
            false,
        ),
        (
            compile::CompilationUnitMode::Merged,
            vec![consumer, provider],
            false,
        ),
    ] {
        let output = compile::compile(&compile::CompileOpts {
            files,
            top: Some("tb".to_owned()),
            compilation_unit_mode: mode,
            ..Default::default()
        })
        .expect("capture compilation-unit visibility");
        assert_eq!(output.ok(), accepted, "{mode:?}: {:?}", output.diagnostics);
        if !accepted {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("UNIT_WIDTH")
                        && diagnostic.line > 0
                        && diagnostic.col > 0
                }),
                "{:?}",
                output.diagnostics
            );
        }
    }
}
