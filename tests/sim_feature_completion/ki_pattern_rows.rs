//! Positional pattern lvalues with unpacked-row targets (IEEE 1800-2009
//! 10.10) and overloaded updates in `for` steps (12.7.1, 11.11). Expected
//! outputs are hand-derived; see the fixture readme.

use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/ki_pattern_rows";

#[test]
fn positional_patterns_scatter_small_sources_into_rows() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/dense_rows.out");
    sim_cli::run_case(SUITE, "dense_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "dense_rows", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "dense_rows", expected);
}

#[test]
fn positional_patterns_copy_descriptor_rows_without_flattening() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/descriptor_rows.out");
    sim_cli::run_case(SUITE, "descriptor_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_rows", expected, &[], &[]);
}

#[test]
fn continuous_positional_patterns_drive_rows() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/continuous_rows.out");
    sim_cli::run_case(SUITE, "continuous_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "continuous_rows", expected, &[], &[]);
}

#[test]
fn neg_row_pattern_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_row_shape",
        "value of type 'logic[7:0]$[4]' cannot be assigned to type 'logic[7:0]$[3]'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_runtime_continuous",
        "continuous assignment-pattern LHS in `tb` requires constant select indices",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_automatic_nba",
        "nonblocking assignment to an automatic assignment-pattern target in `tb` is not supported",
    );
}

/// IEEE 1800-2009 12.7.1: a for step discards its value, so an overloaded
/// update there takes the statement-position form for every target kind.
#[test]
fn for_step_overloaded_updates_run_in_statement_position() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/for_step_updates.out");
    sim_cli::run_case(SUITE, "for_step_updates", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "for_step_updates", expected, &[], &[]);
}

#[test]
fn for_step_side_effect_selectors_keep_the_descriptor_limit() {
    sim_cli::reject_case(
        SUITE,
        "limit_descriptor_step_selector",
        "an overloaded operator update on a target above the 1048575-bit packed value limit or with native members yields a value only as the right-hand side of an assignment, needs side-effect-free target selectors",
    );
}

/// Count the owned overloaded updates of the bound functions in `fixture`:
/// once-bound `OverloadUpdate`/`OverloadPostUpdate` operations whose call
/// reads the target through `OverloadCurrent`, and plain `A = f(A, ...)`
/// assignments that re-read the target.
fn owned_update_counts(fixture: &str, functions: &[&str]) -> (usize, usize, usize) {
    use llg::core::{
        compile,
        db::{Db, ExprKind, NodeKind, Operation},
    };

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    sim_harness::with_frontend_temp_cwd("ki-pattern-rows-db", |_| {
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        let reads_current = |call| {
            let Some(mut argument) = database.node(call).children().first().copied() else {
                return false;
            };
            while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = database.node_kind(argument)
            {
                argument = *operand;
            }
            matches!(
                database.node_kind(argument),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::OverloadCurrent,
                    ..
                })
            )
        };
        let (mut updates, mut post_updates, mut plain) = (0, 0, 0);
        for id in database.node_ids() {
            let NodeKind::Expr(ExprKind::Operation {
                op,
                assignment: true,
                operands,
                ..
            }) = database.node_kind(id)
            else {
                continue;
            };
            let call = operands.get(1).copied().filter(|call| {
                matches!(database.node_kind(*call), NodeKind::FuncCall { name, .. }
                    if functions.contains(&name.as_str()))
            });
            match (*op, call) {
                (Operation::OverloadUpdate, Some(call)) if reads_current(call) => updates += 1,
                (Operation::OverloadPostUpdate, Some(call)) if reads_current(call) => {
                    post_updates += 1
                }
                (Operation::OverloadUpdate | Operation::OverloadPostUpdate, _) => {
                    return Err(format!("update {id:?} does not call through its target"));
                }
                (Operation::Assignment, Some(call)) if !reads_current(call) => plain += 1,
                _ => {}
            }
        }
        Ok((updates, post_updates, plain))
    })
    .expect("checked compilation and owned capture")
}

/// The frontend gives every side-effect-free `for` step the statement form:
/// eight steps and six expression statements re-read their targets, and
/// only the value step `y = x++` binds its target once.
#[test]
fn for_steps_are_captured_as_statement_position_updates() {
    let functions = ["inc", "add", "ninc", "ndec", "nadd", "vinc", "vadd"];
    assert_eq!(
        owned_update_counts("for_step_updates", &functions),
        (0, 1, 14)
    );
}
