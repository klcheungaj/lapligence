use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/rtl_101b";

fn fixture_location(fixture: &str) -> String {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/fixtures/sim/feature_completion/rtl_101b/{fixture}.sv"
    ));
    sim_harness::source_display(&source)
}

#[test]
fn column_records_run_declaration_and_member_initializers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_101b/static_initializers.out");
    sim_cli::run_case(SUITE, "static_initializers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "static_initializers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "static_initializers", expected);
}

#[test]
fn whole_tagged_union_members_are_checked_against_the_tag() {
    let location = fixture_location("tagged_member_guards");
    let expected_stderr = [(21, 11, "w"), (23, 5, "w"), (25, 23, "w"), (26, 9, "s"), (28, 5, "s")]
        .iter()
        .map(|(line, column, member)| {
            format!(
                "llg: runtime error: access to inactive tagged-union member {member} at {location}:{line}:{column}\n"
            )
        })
        .collect::<String>();
    sim_cli::run_case_checked_matrix(SUITE, "tagged_member_guards", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        // An inactive read yields the member's uninitialized value and an
        // inactive write stores nothing; active members copy normally.
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "A x 4\nB 4\nC x\nD x xx\nE 4\nF 1 1\nG 5 07\n",
            "{label}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    });
}

#[test]
fn record_calls_compare_and_select_inside_expressions() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_101b/record_call_operands.out");
    sim_cli::run_case(SUITE, "record_call_operands", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "record_call_operands", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_call_operands", expected);
}

#[test]
fn inactive_members_of_record_call_results_report_runtime_errors() {
    let location = fixture_location("call_result_guard");
    let expected_stderr = format!(
        "llg: runtime error: access to inactive tagged-union member w at {location}:12:23\n"
    );
    sim_cli::run_case_checked_matrix(SUITE, "call_result_guard", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "A x\nB 3\n",
            "{label}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    });
}

#[test]
fn pattern_variables_bind_whole_values_beyond_packed_capacity() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101b/whole_bindings.out");
    sim_cli::run_case(SUITE, "whole_bindings", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "whole_bindings", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "whole_bindings", expected);
}

#[test]
fn neg_column_record_follow_up_limits() {
    sim_cli::reject_case(
        SUITE,
        "neg_nonuniform_member_initializer",
        "member initializer of `a` gives the column's cells different values; column layout keeps one element default",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_native_ref_formal",
        "ref formal `v` of a column-layout record with real, string or chandle members is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_native_output_in_expression",
        "output record argument for `o` in `tb` with real, string or chandle members must be a subroutine record of the same type unless the call is a statement",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_call_member_array",
        "selection `w` of a column-layout record call result in `tb` must name a scalar member or a member array element",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_call_string_member",
        "a string member of a function result in `tb` must be read from a record variable holding the result",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_whole_binding",
        "a whole-value pattern binding beyond packed capacity in `tb` must be part of a procedural statement",
    );
}

#[test]
fn column_records_with_native_members_cross_subroutines() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101b/native_members.out");
    sim_cli::run_case(SUITE, "native_members", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "native_members", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_members", expected);
}

#[test]
fn component_call_member_selections_import_with_their_call() {
    use llg::core::compile;
    use llg::core::db::{Db, ExprKind, NodeKind};
    let source = "module tb;\n\
        typedef struct { logic [7:0] a [0:3]; logic [3:0] k; } in_t;\n\
        typedef struct { in_t s; logic [7:0] w [0:65536]; } rec_t;\n\
        rec_t r;\n\
        logic [7:0] y;\n\
        function automatic rec_t f(input rec_t x); return x; endfunction\n\
        initial begin y = f(r).w[3]; y = {4'h0, f(r).s.k}; end\n\
        endmodule\n";
    let compiled = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "call_members.sv",
            source,
        )],
        &compile::CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("call member selections compile");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    drop(compiled);
    database.validate().expect("database validates");
    let call = |node| matches!(database.node_kind(node), NodeKind::FuncCall { .. });
    let selections = database
        .node_ids()
        .filter_map(|id| match database.node_kind(id) {
            NodeKind::Expr(ExprKind::MemberSelect { base, member }) => {
                Some((*base, member.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    // `f(r).w`, and `f(r).s.k` as `k` of `s` of the call.
    assert!(selections
        .iter()
        .any(|(base, member)| member == "w" && call(*base)));
    let inner = selections
        .iter()
        .find(|(base, member)| member == "s" && call(*base))
        .map(|(base, _)| *base)
        .expect("`s` of the call");
    assert!(selections.iter().any(|(base, member)| member == "k"
        && matches!(database.node_kind(*base),
            NodeKind::Expr(ExprKind::MemberSelect { base, member }) if member == "s" && *base == inner)));
    // The element select keeps the member selection as its base.
    assert!(database.node_ids().any(|id| matches!(
        database.node_kind(id),
        NodeKind::Expr(ExprKind::ArraySelect { base, indices })
            if indices.len() == 1
                && matches!(database.node_kind(*base),
                    NodeKind::Expr(ExprKind::MemberSelect { member, .. }) if member == "w")
    )));
}
