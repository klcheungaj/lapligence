//! RTL-104: operator-overload leftovers (IEEE 1800-2009 11.11): values of
//! overloaded increments, targets evaluated once, expected types from a
//! relational operand, and the project ruling on package-import visibility.
//! Expected outputs are hand-derived; see the fixture readme.

use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/rtl_104";
const AMBIGUOUS_PLUS: &str =
    "ambiguous overload of operator '+'; use a cast to select the result type";
const INVALID_PLUS: &str = "invalid operands to binary expression ('p::s_t' and 'p::s_t')";

#[test]
fn increment_values_follow_prefix_and_postfix_rules() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/incdec_values.out");
    sim_cli::run_case(SUITE, "incdec_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "incdec_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "incdec_values", expected);
}

#[test]
fn update_targets_are_evaluated_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/single_evaluation.out");
    sim_cli::run_case(SUITE, "single_evaluation", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "single_evaluation", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "single_evaluation", expected);
}

#[test]
fn former_rtl_017_negatives_execute() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/rtl017_postfix_value.out");
    sim_cli::run_case(SUITE, "rtl017_postfix_value", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "rtl017_postfix_value", expected, &[], &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/rtl017_target_side_effects.out");
    sim_cli::run_case(SUITE, "rtl017_target_side_effects", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "rtl017_target_side_effects", expected, &[], &[]);
}

#[test]
fn relational_operands_supply_the_expected_type() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/relational_expected.out");
    sim_cli::run_case(SUITE, "relational_expected", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "relational_expected", expected, &[], &[]);
}

#[test]
fn package_overloads_follow_wildcard_imports() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/package_import.out");
    sim_cli::run_case(SUITE, "package_import", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "package_import", expected, &[], &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/package_import_unit.out");
    sim_cli::run_case(SUITE, "package_import_unit", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "package_import_unit", expected, &[], &[]);
}

#[test]
fn overloaded_updates_compose_with_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
}

#[test]
fn neg_comparisons_without_one_unambiguous_operand_need_a_cast() {
    sim_cli::reject_case(SUITE, "neg_relational_both_ambiguous", AMBIGUOUS_PLUS);
    sim_cli::reject_case(SUITE, "neg_relational_nested", AMBIGUOUS_PLUS);
    sim_cli::reject_case(SUITE, "neg_relational_no_match", AMBIGUOUS_PLUS);
}

#[test]
fn neg_package_overloads_need_a_preceding_wildcard_import() {
    sim_cli::reject_case(SUITE, "neg_import_explicit", INVALID_PLUS);
    sim_cli::reject_case(SUITE, "neg_import_after_use", INVALID_PLUS);
    sim_cli::reject_case(
        SUITE,
        "neg_import_reexport",
        "invalid operands to binary expression ('p::s_t' and 'p::s_t')",
    );
    sim_cli::reject_case(SUITE, "neg_import_two_packages", AMBIGUOUS_PLUS);
}

#[test]
fn native_update_values_report_their_limit() {
    sim_cli::reject_case(
        SUITE,
        "limit_native_value",
        "an overloaded operator update on a target above the 1048575-bit packed value limit or with native members yields a value only as the right-hand side of an assignment",
    );
}

/// Count a fixture's once-bound overloaded updates (prefix-valued, postfix)
/// and its statement updates kept as `A = f(A)` assignments, checking that
/// every once-bound update calls its function through an `OverloadCurrent`
/// operand rather than a second binding of the target.
fn owned_update_counts(fixture: &str) -> (usize, usize, usize) {
    use llg::core::{
        compile,
        db::{Db, ExprKind, NodeKind, Operation},
    };

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    sim_harness::with_frontend_temp_cwd("rtl-104-db", |_| {
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
            let (op, call) = match database.node_kind(id) {
                NodeKind::Expr(ExprKind::Operation {
                    op,
                    assignment: true,
                    operands,
                    ..
                }) => (*op, operands.get(1).copied()),
                // An expression statement's assignment node is its child too.
                _ => continue,
            };
            let call = call.filter(|call| {
                matches!(database.node_kind(*call), NodeKind::FuncCall { name, .. }
                    if matches!(name.as_str(), "inc" | "add"))
            });
            match (op, call) {
                (Operation::OverloadUpdate | Operation::OverloadPostUpdate, Some(call))
                    if reads_current(call) =>
                {
                    if op == Operation::OverloadUpdate {
                        updates += 1;
                    } else {
                        post_updates += 1;
                    }
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

#[test]
fn once_bound_updates_are_owned_update_operations() {
    // `arr[next()] += d`, `++arr[next()]` and `y = (arr[next()] += d)`;
    // `arr[next()]++`, `y = arr[next()]++`, `recs[next()].inner++` and the
    // `for` step `arr[next()]++`.
    assert_eq!(owned_update_counts("single_evaluation"), (3, 4, 0));
    // `z = x + y`, `z++`, `b = a + a` and `b++` keep ordinary assignments.
    assert_eq!(owned_update_counts("package_import"), (0, 0, 4));
}
