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
fn neg_column_record_follow_up_limits() {
    sim_cli::reject_case(
        SUITE,
        "neg_nonuniform_member_initializer",
        "member initializer of `a` gives the column's cells different values; column layout keeps one element default",
    );
}
