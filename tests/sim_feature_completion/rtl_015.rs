use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_015";

/// Run a fixture whose run-time streaming errors make it exit 1. Its results
/// are on stderr, interleaved with the reports; compare that stream exactly
/// in both HDL optimizer modes and on every available value backend.
fn assert_runtime_errors(fixture: &str, expected: &str) {
    let gmp = std::env::var("LLG_TEST_GMP_ROOT").unwrap_or_default();
    for optimized in [false, true] {
        for (backend, kernel) in [
            ("legacy", "portable"),
            ("compact", "portable"),
            ("compact", "gmp"),
        ] {
            if kernel == "gmp" && gmp.is_empty() {
                eprintln!("BLOCKED GMP parity: set LLG_TEST_GMP_ROOT");
                continue;
            }
            let controls = [
                ("LLG_VALUE_BACKEND", backend),
                ("LLG_COMPACT_KERNELS", kernel),
                ("GMP_ROOT", gmp.as_str()),
            ];
            let output = sim_cli::invoke_with_env(SUITE, fixture, optimized, &[], &controls, &[]);
            let label = format!("{fixture}, {backend}/{kernel}, optimized={optimized}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            let runtime: String = stderr
                .lines()
                .filter(|line| !line.starts_with("Warning: "))
                .map(|line| format!("{line}\n"))
                .collect();
            assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
            assert!(output.stdout.is_empty(), "{label}: {output:?}");
            assert_eq!(runtime, expected, "{label}");
        }
    }
}

#[test]
fn fixed_streams_pack_unpack_and_cast_in_every_context() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_015/pack_unpack.out");
    sim_cli::run_case(SUITE, "pack_unpack", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "pack_unpack", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "pack_unpack", expected);
}

#[test]
fn runtime_with_ranges_follow_storage_order_and_defaults() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_015/runtime_with.out");
    sim_cli::run_case(SUITE, "runtime_with", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "runtime_with", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "runtime_with", expected);
}

#[test]
fn with_ranges_select_ref_local_member_row_and_call_views() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_015/with_views.out");
    sim_cli::run_case(SUITE, "with_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "with_views", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "with_views", expected);
}

#[test]
fn queued_fixed_streams_keep_issue_time_selectors_and_sources() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_015/queued.out");
    sim_cli::run_case(SUITE, "queued", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "queued", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "queued", expected);
}

#[test]
fn out_of_bounds_with_targets_write_the_valid_part_and_report() {
    let report =
        "llg: fixed streaming target selector is unknown, empty, or outside declared bounds\n";
    assert_runtime_errors(
        "target_bounds",
        &format!(
            "{report}static 00 00 aa bb\n\
             {report}queued 22 00 aa bb\n\
             {report}unknown 00 00\n"
        ),
    );
    sim_cli::reject_case(SUITE, "target_bounds", "outside declared bounds");
}

#[test]
fn oversized_constant_with_ranges_stay_descriptor_streams() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_015/descriptor_with.out");
    sim_cli::run_case(SUITE, "descriptor_with", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_with", expected, &[], &[]);
}

#[test]
fn neg_stream_compound() {
    // Adopted FND-002 witness for L-F07-15-03 (SV2009 Annex A.6.2): a
    // streaming concatenation is not an operator-assignment lvalue.
    sim_cli::reject_case(
        SUITE,
        "neg_stream_compound",
        "streaming operator can only be used in an assignment or bit-stream cast argument",
    );
}

#[test]
fn neg_fixed_size_mismatches() {
    sim_cli::reject_case(
        SUITE,
        "neg_cast_undersized",
        "cannot be converted to type 'nib_t'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_pack_oversized",
        "streaming operator target size 32 does not fit source size 96",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_unpack_undersized",
        "streaming operator target size 96 does not fit source size 23",
    );
    sim_cli::reject_case(SUITE, "neg_union_cast", "cannot be converted to type 'u_t'");
}

#[test]
fn neg_selector_reading_an_earlier_target() {
    sim_cli::reject_case(
        SUITE,
        "neg_nba_with_dependence",
        "nonblocking streaming `with` selector reads a target unpacked earlier",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_reversed_with_dependence",
        "right-to-left streaming `with` selector reads a target unpacked earlier",
    );
}

#[test]
fn neg_unrepresented_with_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_mixed_state_target",
        "streaming `with` target elements mixing two-state and four-state members are not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_with_output",
        "requires a direct streaming assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_descriptor_runtime_with",
        "descriptor stream `with` range must be constant and inside the array bounds",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_nba_container",
        "nonblocking assignment to a streaming container target",
    );
}
