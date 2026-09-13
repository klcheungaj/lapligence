//! End-to-end target-edition and ordinary time-literal acceptance tests.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile::{self, CompileOpts, LanguageEdition, OwnedSource};
use llg::core::db::Db;
use std::path::Path;

#[test]
fn systemverilog_2009_rounds_time_literals_before_value_use() {
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_rounding_2009",
        "CHECK: literal=2.0\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_values_2009",
        concat!(
            "value=1.50 initialized=1.50 positive=1.60 negative=-1.60 arithmetic=1.50 parameter=1.50\n",
            "delay=1.60\n"
        ),
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn systemverilog_2009_preserves_exact_femtosecond_literal_boundaries() {
    sim_cli::run_case_with_args(
        "partial_features",
        "time_literal_exact_2009",
        "exact=1.000001 -1.000001 0.000001 delay=1.000001\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn declaration_initialization_keeps_edition_specific_scheduling() {
    sim_cli::run_case_with_args(
        "partial_features",
        "declaration_init_edition",
        "PASS declaration_init_edition\n",
        "llg: $finish at time 0 at tb:15:9\n",
        &[],
        &["--edition", "2001"],
    );
    sim_cli::run_case_with_args(
        "partial_features",
        "declaration_init_edition",
        "PASS declaration_init_edition\n",
        "llg: $finish at time 0 at tb:15:9\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn verilog_2001_rejects_systemverilog_constructs() {
    sim_cli::reject_case_with_args(
        "partial_features",
        "edition_2001_sv_only",
        "always_comb",
        &["--edition", "2001"],
    );
}

#[test]
fn begin_keywords_does_not_change_the_selected_global_edition() {
    let output = compile::compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "begin-keywords.sv",
            "`begin_keywords \"1800-2009\"\nmodule tb; reg value; initial value = 1'b0; endmodule\n`end_keywords\n",
        )],
        top: Some("tb".to_owned()),
        edition: LanguageEdition::Verilog2001,
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    assert!(!output.snapshot.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.snapshot.edition(), LanguageEdition::Verilog2001);
    let database = Db::from_slang(&output.snapshot).expect("owned database");
    assert_eq!(database.edition(), LanguageEdition::Verilog2001);
}

#[test]
fn owned_model_retains_parameter_override_provenance() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/edition_parameter_override.sv");
    let output = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        param_overrides: vec!["P=2".to_owned()],
        ..CompileOpts::default()
    })
    .expect("bridge compile");
    let database = Db::from_slang(&output.snapshot).expect("owned database");
    let parameter = database
        .node_ids()
        .find(|id| {
            matches!(
                database.node_kind(*id),
                llg::core::db::NodeKind::Param { .. }
            )
        })
        .expect("captured parameter");
    assert!(database.parameter_is_overridden(parameter));
}
