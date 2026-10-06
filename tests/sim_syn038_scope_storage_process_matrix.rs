//! SYN-038 storage, subroutine and hierarchy paths across process contexts.

use crate::sim_cli;

use llg::core::{
    compile::{self, CompileOpts, LanguageEdition},
    db::{Db, ExprKind, NodeId, NodeKind, StmtKind, VariableLifetime},
};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv";
const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv");
const EXPECTED_STDOUT: &str = "scope=5b,5b,5b,5b,5b,5a,5b event=5a gen=1,1 if=1,1 const=5\n";
const FOCAL_SOURCE_ANCHORS: &[&str] = &[
    "interface process_if(input logic source);",
    "assign net_value = source;",
    "assign variable_value = source;",
    "for (genvar g = 0; g < 1; g++) begin : generated",
    "assign gen_net = source[0];",
    "gen_local = 1'b1;",
    "always @(trig) begin : plain_process",
    "static logic [7:0] saved;",
    "automatic logic [7:0] local_value = source;",
    "saved = local_value;",
    "plain_result = identity(saved);",
    "plain_nba_result <= saved;",
    "always_comb begin : comb_process",
    "comb_result = identity(saved);",
    "always_latch begin : latch_process",
    "latch_result = identity(saved);",
    "always_ff @(posedge clk) begin : ff_process",
    "static logic [7:0] blocking_saved;",
    "static logic [7:0] nba_saved;",
    "blocking_saved = source;",
    "nba_saved <= source;",
    "ff_block_result <= blocking_saved;",
    "ff_nba_result <= nba_saved;",
    "function automatic logic [7:0] identity(input logic [7:0] value);",
    "return value;",
    "function automatic int constant_identity(input int value);",
    "automatic int per_call = value;",
    "return per_call;",
    "localparam int CONST_SOURCE = 5;",
    "localparam int CONST_RESULT = constant_identity(CONST_SOURCE);",
    "@(posedge trig);",
    "event_seen = source;",
    "if (plain_result !== 8'h5a || plain_nba_result !== 8'h5a ||",
    "if (plain_result !== 8'h5b || plain_nba_result !== 8'h5b ||",
];

fn compile_fixture() -> Db {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let compiled = compile::compile_checked(&CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".into()),
        edition: LanguageEdition::SystemVerilog2009,
        ..Default::default()
    })
    .expect("legal SV2009 scope and process fixture compiles");
    Db::from_slang(&compiled.snapshot).expect("capture owned semantic database")
}

fn source_line(anchor: &str) -> u32 {
    let matches = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .filter_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one source anchor `{anchor}`");
    matches[0]
}

fn source_line_after(after_anchor: &str, anchor: &str) -> u32 {
    let after_line = source_line(after_anchor);
    FIXTURE_SOURCE
        .lines()
        .enumerate()
        .skip(after_line as usize)
        .find_map(|(line, text)| (text.trim() == anchor).then_some(line as u32 + 1))
        .unwrap_or_else(|| panic!("fixture is missing `{anchor}` after `{after_anchor}`"))
}

fn declaration_at_line(db: &Db, name: &str, line: u32) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).name() == name
                && db.node(*id).line() == line
                && matches!(db.node_kind(*id), NodeKind::Var { .. })
        })
        .unwrap_or_else(|| panic!("line {line} declares variable `{name}`"))
}

fn assignment_at_line(db: &Db, line: u32) -> NodeId {
    db.node_ids()
        .find(|id| {
            db.node(*id).line() == line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::Stmt(StmtKind::Assign { blocking: true, .. })
                )
        })
        .unwrap_or_else(|| panic!("line {line} has a blocking assignment"))
}

fn direct_reference_target(db: &Db, expression: NodeId) -> NodeId {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) => *target,
        other => panic!("expected a bound direct reference, got {other:?}"),
    }
}

#[test]
fn storage_and_process_contexts_keep_distinct_source_paths_observable() {
    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv\n"
    ));
    for anchor in FOCAL_SOURCE_ANCHORS {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost source-path anchor: {anchor}"
        );
    }
    for oracle in ["$fatal(1, \"phase one\")", "$fatal(1, \"phase two\")"] {
        assert!(
            FIXTURE_SOURCE.contains(oracle),
            "fixture lost independent phase check: {oracle}"
        );
    }

    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "scope_storage_process_matrix",
        EXPECTED_STDOUT,
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn static_process_writer_and_function_actual_share_owned_source_identity() {
    let db = compile_fixture();
    let process_start = "always @(trig) begin : plain_process";
    let saved = declaration_at_line(
        &db,
        "saved",
        source_line_after(process_start, "static logic [7:0] saved;"),
    );
    let local_value = declaration_at_line(
        &db,
        "local_value",
        source_line_after(process_start, "automatic logic [7:0] local_value = source;"),
    );
    let result = declaration_at_line(
        &db,
        "plain_result",
        source_line("logic [7:0] plain_result;"),
    );
    assert_eq!(
        db.variable_lifetime(saved),
        VariableLifetime::Static,
        "the focal process-local declaration has static lifetime"
    );

    let writer = assignment_at_line(
        &db,
        source_line_after(process_start, "saved = local_value;"),
    );
    let writer_children = db.node(writer).children();
    assert_eq!(writer_children.len(), 2, "writer retains LHS and RHS");
    let writer_target = direct_reference_target(&db, writer_children[0]);
    let writer_source = direct_reference_target(&db, writer_children[1]);
    assert_eq!(db.source_identity(writer_target), db.source_identity(saved));
    assert_eq!(
        db.source_identity(writer_source),
        db.source_identity(local_value)
    );

    let call_line = source_line_after(process_start, "plain_result = identity(saved);");
    let calls = db
        .node_ids()
        .filter(|id| {
            db.node(*id).line() == call_line
                && matches!(
                    db.node_kind(*id),
                    NodeKind::FuncCall {
                        name,
                        is_task: false,
                        ..
                    } if name == "identity"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "assignment RHS contains one identity call");
    let actuals = db.node(calls[0]).children();
    assert_eq!(actuals.len(), 1, "identity receives one actual");
    let actual_source = direct_reference_target(&db, actuals[0]);
    assert_eq!(
        db.source_identity(writer_target),
        db.source_identity(actual_source),
        "blocking writer LHS and function actual resolve to the same saved declaration"
    );
    assert_ne!(
        db.source_identity(actual_source),
        db.source_identity(writer_source),
        "the call reads static saved rather than its distinct automatic writer input"
    );
    assert_ne!(
        db.source_identity(actual_source),
        db.source_identity(result),
        "the function actual remains distinct from the assignment result"
    );
}
