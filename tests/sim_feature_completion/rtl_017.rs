//! RTL-017: fixed operator-overload resolution and execution (IEEE 1800-2009
//! 11.11, A.2.8). Expected outputs are hand-derived; see the fixture readme.

use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/rtl_017";
const NO_FINISH: &str = "llg: simulation ended without $finish (no processes remain) at time 0\n";

#[test]
fn saturating_records_overload_every_operator_form() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_017/saturating_records.out");
    sim_cli::run_case(SUITE, "saturating_records", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "saturating_records", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "saturating_records", expected);
}

#[test]
fn result_types_are_selected_by_context_or_cast() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_017/result_casts.out");
    let stderr = "llg: $finish at time 2000 at tb:63:8\n";
    sim_cli::run_case(SUITE, "result_casts", expected, stderr, &[]);
    sim_cli::run_case_backend_parity(SUITE, "result_casts", expected, &[], &[]);
}

#[test]
fn declarations_follow_scope_order_and_shadowing() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_017/scopes.out");
    sim_cli::run_case(SUITE, "scopes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "scopes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "scopes", expected);
}

#[test]
fn fixed_point_records_compose_with_ports_and_processes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_017/fixed_point_pipeline.out");
    let stderr = "llg: $finish at time 5000 at tb:66:5\n";
    sim_cli::run_case(SUITE, "fixed_point_pipeline", expected, stderr, &[]);
    sim_cli::run_case_backend_parity(SUITE, "fixed_point_pipeline", expected, &[], &[]);
}

#[test]
fn oversized_array_operands_cross_overloads_as_descriptors() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_017/descriptor_operands.out");
    let stderr = "llg: $finish at time 0 at tb:46:5\n";
    sim_cli::run_case(SUITE, "descriptor_operands", expected, stderr, &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_operands", expected, &[], &[]);
}

#[test]
fn legal_builtin_operations_keep_their_meaning() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_017/builtin_preserved.out");
    sim_cli::run_case(SUITE, "builtin_preserved", expected, NO_FINISH, &[]);
    sim_cli::run_case_backend_parity(SUITE, "builtin_preserved", expected, &[], &[]);
}

#[test]
fn operator_overload_witness() {
    // Adopted FND-002 witness for L-F07-17-01 (SV2009 11.11, A.2.8).
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_017/operator_overload_witness.out");
    let stderr = "llg: $finish at time 0 at tb:7:1\n";
    sim_cli::run_case(SUITE, "operator_overload_witness", expected, stderr, &[]);
    sim_cli::run_case_backend_parity(SUITE, "operator_overload_witness", expected, &[], &[]);
}

/// The owned Db receives each resolved overload as an ordinary call to the
/// function bound from the use's scope, so lowering reuses the call ABI.
#[test]
fn resolved_overloads_are_owned_calls_to_bound_functions() {
    use llg::core::{compile, db::Db, db::NodeKind};

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("scopes.sv");
    let calls = sim_harness::with_frontend_temp_cwd("rtl-017-db", |_| {
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        let mut calls = Vec::new();
        for id in database.node_ids() {
            if let NodeKind::FuncCall {
                name,
                callee: Some(callee),
                ..
            } = database.node_kind(id)
            {
                let owner = database
                    .node(*callee)
                    .parent()
                    .map(|parent| database.node(parent).name.clone())
                    .unwrap_or_default();
                calls.push(format!("{name}@{owner}"));
            }
        }
        calls.sort();
        Ok(calls)
    })
    .expect("checked compilation and owned capture");
    // Instance `o` of `other` binds the $unit `+` to its own `add1`; tb binds
    // it to the compilation-unit one, which has no named owner.
    assert!(calls.contains(&"add1@o".to_owned()), "{calls:?}");
    for bound in ["add2", "add3", "unit_mul"] {
        assert!(
            calls
                .iter()
                .any(|call| call.starts_with(&format!("{bound}@"))),
            "{bound}: {calls:?}"
        );
    }
    assert_eq!(
        calls.iter().filter(|call| *call == "add1@").count(),
        2,
        "{calls:?}"
    );
}

#[test]
fn self_determined_results_need_a_cast() {
    sim_cli::reject_case(
        SUITE,
        "neg_self_determined_ambiguous",
        "ambiguous overload of operator '+'; use a cast to select the result type",
    );
}

#[test]
fn nested_results_need_a_cast() {
    sim_cli::reject_case(
        SUITE,
        "neg_nested_ambiguous",
        "ambiguous overload of operator '+'; use a cast to select the result type",
    );
}

#[test]
fn declarations_are_not_visible_before_they_are_declared() {
    sim_cli::reject_case(
        SUITE,
        "neg_use_before_declaration",
        "invalid operands to binary expression ('T' and 'T')",
    );
}

#[test]
fn declarations_are_not_visible_in_sibling_blocks() {
    sim_cli::reject_case(
        SUITE,
        "neg_nonvisible_block",
        "invalid operands to binary expression ('T' and 'T')",
    );
}

#[test]
fn bound_functions_must_be_visible() {
    sim_cli::reject_case(
        SUITE,
        "neg_missing_function",
        "operator overload declaration binds 'no_such_add', which is not a visible function",
    );
}

#[test]
fn bound_functions_must_match_their_prototype() {
    sim_cli::reject_case(
        SUITE,
        "neg_prototype_mismatch",
        "function 'addi' does not match its operator overload prototype",
    );
}

#[test]
fn prototypes_must_have_the_operator_arity() {
    sim_cli::reject_case(
        SUITE,
        "neg_overload_arity",
        "operator '++' cannot be overloaded with 2 formal argument(s)",
    );
}

#[test]
fn only_overload_operators_can_be_bound() {
    sim_cli::reject_case(SUITE, "neg_not_overloadable", "expected identifier");
}

#[test]
fn verilog_2001_rejects_overload_declarations() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_v2001_overload.v",
        "neg_v2001_overload.v:7:8 expected a declaration name",
        &["--edition", "v2001"],
    );
}
