//! SIM-020: legal native and resizable bit-stream operations (IEEE 1800-2009
//! 6.24.3, 11.4.14). Positive fixtures run in both optimizer modes on the
//! legacy, compact/portable and compact/GMP value backends against
//! hand-derived outputs; see the fixture readme for the clauses.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_020";

#[test]
fn strings_queues_and_unpacked_arrays_round_trip() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_020/roundtrip.out");
    sim_cli::run_case_backend_parity(SUITE, "roundtrip", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "roundtrip", expected);
}

#[test]
fn dynamic_extents_with_ranges_and_greedy_targets() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_020/dynamic_extents.out");
    sim_cli::run_case_backend_parity(SUITE, "dynamic_extents", expected, &[], &[]);
}

#[test]
fn overlapping_operands_and_selectors_keep_capture_order() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_020/capture_order.out");
    sim_cli::run_case_backend_parity(SUITE, "capture_order", expected, &[], &[]);
}

#[test]
fn streams_beyond_the_packed_width_stay_segmented() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_020/large.out");
    sim_cli::run_case_backend_parity(SUITE, "large", expected, &[], &[]);
}

#[test]
fn resizable_operands_and_targets_of_oversized_streams() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_020/oversized_mixed.out");
    sim_cli::run_case_backend_parity(SUITE, "oversized_mixed", expected, &[], &[]);
}

/// A size error found at run time stops the run after the fixture's
/// `before` line, on every backend, before any target is written.
fn size_error<'a>(
    stdout: &'a str,
    diagnostic: &'a str,
) -> impl Fn(&str, &std::process::Output) + 'a {
    move |label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{label}: {output:?}"
        );
    }
}

const SHORT_SOURCE: &str = "llg container fatal: streaming unpack source has insufficient bits";

#[test]
fn dynamic_size_mismatches_fail_at_run_time() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "err_short_source",
        &[],
        &size_error("before 5\n", SHORT_SOURCE),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "err_cast_elements",
        &[],
        &size_error(
            "before 3\n",
            "llg container fatal: bit stream size does not match a whole number of destination elements",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "err_cast_fixed",
        &[],
        &size_error(
            "before 7\n",
            "llg container fatal: bit-stream cast source size does not match its fixed-size target",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "err_oversized_short",
        &[],
        &size_error("before 2\n", SHORT_SOURCE),
    );
}

#[test]
fn illegal_bit_stream_types_and_targets_stay_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_assoc_target",
        "stream expression type 'byte$[int]' is not a bit-stream type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_assoc_cast",
        "cannot be converted to type 'ab_t' (aka 'byte$[int]')",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_member",
        "stream expression type 's_t' is not a bit-stream type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_chandle",
        "stream expression type 'chandle' is not a bit-stream type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_event",
        "stream expression type 'event' is not a bit-stream type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_queue",
        "stream expression type 'real$[$]' is not a bit-stream type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fixed_cast_size",
        "cannot be converted to type 'b3_t' (aka 'byte$[3]')",
    );
}

#[test]
fn illegal_nonblocking_stream_targets_stay_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_nba",
        "nonblocking assignment to automatic variable 'f' is not allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_element_nba",
        "nonblocking assignments to elements of dynamically sized arrays are not allowed",
    );
}

#[test]
fn unsupported_legal_forms_are_explicit_rejections() {
    sim_cli::reject_case(
        SUITE,
        "unsupported_class_stream",
        "streaming a class object's members is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "unsupported_dynamic_record_cast",
        "bit-stream cast into a struct with string or resizable members is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "unsupported_dynamic_nba",
        "nonblocking assignment to a streaming container target",
    );
}
