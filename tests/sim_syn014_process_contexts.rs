//! SYN-014 public-pipeline evidence for aggregate sensitivity and always-family contracts.

use crate::sim_cli;

#[test]
fn aggregate_sensitivity_and_always_family_contracts_match_in_both_modes() {
    sim_cli::run_case(
        "syn014_process_contexts",
        "process_contexts",
        concat!(
            "aggregate arm=1 at=2 feedback=c linked=07 split=c1 latch=x ff=00 delta=1\n",
            "enabled arm=5 at=2 feedback=c linked=15 split=c5 latch=5 ff=00\n",
            "triggered arm=c at=c feedback=c linked=15 split=c5 latch=5 ff=00\n",
            "array arm=c at=c feedback=c linked=31 split=c5 latch=5 ff=00\n",
            "held ff=00\n",
            "edge ff=2a\n",
            "reset ff=00\n",
        ),
        "",
        &[],
    );
}

#[test]
fn overlapping_packed_writers_are_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "overlapping_writers",
        "multiple writers",
    );
}

#[test]
fn extra_always_ff_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "extra_event",
        "one and only one event control",
    );
}

#[test]
fn explicit_always_latch_event_is_rejected_without_lint() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "latch_event",
        "statements that pass time",
    );
}

#[test]
fn repaired_expression_kinds_keep_sensitivity_without_spurious_wakeups() {
    // Independent oracle: SV 9.2.2.2.1 always_comb reads (both conditional
    // arms, called-function arguments, later predicate clauses), 11.4.11
    // structure merge, 11.4.13 array set membership and V 9.7.5 `@*` reads.
    sim_cli::run_case(
        "syn014_process_contexts",
        "repaired_expression_contexts",
        concat!(
            "initial merged=5a/1/12 hit=1 eq=1 matched=40 lanes=cd,ab word=d1 latch=5a reg=xx\n",
            "unselected merged=5a/1/12 hit=1 eq=1 matched=40 lanes=cd,ab word=d1 latch=5a reg=xx\n",
            "selected merged=33/0/7f hit=1 eq=1 matched=40 lanes=cd,ab word=d1 latch=5a reg=xx\n",
            "members merged=33/0/7f hit=0 eq=0 matched=40 lanes=cd,ab word=d1 latch=5b reg=xx\n",
            "predicate merged=33/0/7f hit=1 eq=0 matched=00 lanes=cd,ab word=d1 latch=5b reg=xx\n",
            "contents merged=33/0/7f hit=1 eq=0 matched=40 lanes=cd,ee word=e1 latch=5b reg=xx\n",
            "selector merged=33/0/7f hit=1 eq=0 matched=40 lanes=cd,ee word=d3 latch=5b reg=xx\n",
            "held merged=33/0/7f hit=1 eq=0 matched=40 lanes=cd,ee word=d3 latch=5b reg=xx\n",
            "edge merged=33/0/7f hit=1 eq=0 matched=40 lanes=cd,ee word=d3 latch=5b reg=33\n",
        ),
        "",
        &[],
    );
}

#[test]
fn disjoint_pattern_element_and_member_writers_are_legal() {
    sim_cli::run_case(
        "syn014_process_contexts",
        "disjoint_repaired_writers",
        concat!(
            "row2=aa first=bb row0=11 data=11 flag=0\n",
            "row2=aa first=bb row0=22 data=22 flag=1\n",
        ),
        "",
        &[],
    );
}

#[test]
fn overlapping_repaired_lvalue_writers_are_rejected() {
    sim_cli::reject_case(
        "syn014_process_contexts",
        "pattern_writer_overlap",
        "has multiple writers for `tb.second`",
    );
    sim_cli::reject_case(
        "syn014_process_contexts",
        "member_ff_overlap",
        "has multiple writers for `tb.value.data`",
    );
    sim_cli::reject_case(
        "syn014_process_contexts",
        "pattern_continuous_overlap",
        "variable storage `tb.row[0]` has both a continuous assignment",
    );
}

#[test]
fn rejection_diagnostics_use_source_names_without_c_identifiers() {
    for fixture in [
        "overlapping_writers",
        "extra_event",
        "latch_event",
        "pattern_writer_overlap",
        "member_ff_overlap",
        "pattern_continuous_overlap",
    ] {
        for optimized in [false, true] {
            let output = sim_cli::invoke_with_env(
                "syn014_process_contexts",
                fixture,
                optimized,
                &[],
                &[],
                &[],
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{fixture}: {stderr}");
            assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
            for internal in ["G_", "D_", "cI_", "__llg_ident_"] {
                assert!(!stderr.contains(internal), "{fixture}: {stderr}");
            }
        }
    }
}
