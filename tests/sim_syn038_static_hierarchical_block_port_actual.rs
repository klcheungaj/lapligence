//! SYN-038 static block-local hierarchical port actual.

use llg::core::compile::{self, CompileError, CompileOpts, LanguageEdition, Severity};
use std::path::Path;

use crate::sim_cli;
use crate::sim_harness;

const UNQUALIFIED_FIXTURE: &str =
    "tests/fixtures/sim/syn038_pairwise/static_unqualified_block_port_actual_rejected.sv";
const UNQUALIFIED_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/static_unqualified_block_port_actual_rejected.sv");

fn unqualified_options() -> CompileOpts {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(UNQUALIFIED_FIXTURE);
    CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    }
}

fn source_line(anchor: &str) -> u32 {
    let matches = UNQUALIFIED_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one source anchor `{anchor}`");
    matches[0]
}

#[test]
fn static_block_local_hierarchical_port_actual_runs_in_both_optimizer_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "static_hierarchical_block_port_actual",
        "static_hierarchical_block_port_actual=passed\n",
        "llg: $finish at time 1000 at tb:32:9\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn unqualified_static_block_local_port_actual_is_rejected_by_frontend_and_cli() {
    assert!(UNQUALIFIED_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_unqualified_block_port_actual_rejected.sv\n"
    ));
    for anchor in [
        "leaf u(.value(static_value));",
        "initial begin : named_process",
        "static logic [7:0] static_value;",
        "static_value = 8'h31;",
    ] {
        assert!(
            UNQUALIFIED_SOURCE.contains(anchor),
            "missing unqualified static-local source anchor `{anchor}`"
        );
    }

    let options = unqualified_options();
    let raw = compile::compile(&options).expect("Slang returns the expected source diagnostic");
    assert!(!raw.ok(), "raw frontend result records a blocking error");
    let errors = raw
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.severity,
                Severity::Fatal | Severity::Syntax | Severity::Error
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "the source has one blocking diagnostic");
    assert_eq!(
        errors[0].message,
        "use of undeclared identifier 'static_value'"
    );
    assert_eq!(
        errors[0].line,
        source_line("leaf u(.value(static_value));"),
        "diagnostic points to the module-scope child port actual"
    );
    assert_eq!(
        errors[0].col, 19,
        "diagnostic points to the undeclared name"
    );
    let raw_diagnostics = raw.diagnostics.clone();
    assert!(raw.snapshot.has_errors());
    let checked = compile::compile_checked(&options)
        .expect_err("checked compile rejects the undeclared port-actual source");
    assert!(matches!(&checked, CompileError::FrontendDiagnostics(_)));
    assert_eq!(
        checked
            .diagnostics()
            .expect("frontend diagnostics are retained"),
        raw_diagnostics.as_slice()
    );

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(UNQUALIFIED_FIXTURE);
    let path = sim_harness::source_display(&fixture_path);
    let expected_stderr = format!(
        "Error: {path}:7:19 use of undeclared identifier 'static_value'\n\
Warning: {path}:3:31 unused port signal 'value'\n\
Warning: {path}:10:28 variable 'static_value' is assigned but its value is never used\n\
llg: Slang reported errors; aborting\n"
    );
    sim_cli::reject_case_with_exact_stderr(
        "syn038_pairwise",
        "static_unqualified_block_port_actual_rejected",
        &expected_stderr,
        &["--edition", "sv2009"],
    );
}
