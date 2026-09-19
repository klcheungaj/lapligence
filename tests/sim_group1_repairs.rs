//! Regression coverage for review findings R02-R08, R10-R12 and R15.
//! Public HDL cases run with and without optimization. C runtime endpoint tests
//! are registered separately in tests/runtime_value_storage/CMakeLists.txt.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "group1_repairs";

#[test]
fn callback_loop_labels_are_unique_per_inline_expansion() {
    sim_cli::run_case(SUITE, "callback_loops", "callback loops passed\n", "", &[]);
}

#[test]
fn callback_real_results_survive_their_private_scope() {
    sim_cli::run_case(SUITE, "callback_real", "real callbacks passed\n", "", &[]);
}

#[test]
fn instance_source_indices_match_their_port_values() {
    let expected = [
        "tb.offset_array[5]=10",
        "tb.offset_array[4]=3",
        "tb.negative_array[-2]=1",
        "tb.negative_array[-1]=2",
        "tb.negative_array[0]=3",
        "tb.negative_array[1]=4",
        "tb.ascending_array[0]=5",
        "tb.ascending_array[1]=6",
        "tb.ascending_array[2]=7",
        "tb.ascending_array[3]=8",
        "tb.descending_array[3]=9",
        "tb.descending_array[2]=10",
        "tb.descending_array[1]=11",
        "tb.descending_array[0]=12",
        "tb.matrix[2][0]=1",
        "tb.matrix[2][1]=3",
        "tb.matrix[1][0]=5",
        "tb.matrix[1][1]=7",
    ];
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(SUITE, "instance_indices", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "optimized={optimized}: {stderr}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut actual: Vec<_> = stdout.lines().collect();
        let mut expected = expected.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();
        // Compare path/value PAIRS, never just the set of index spellings.
        assert_eq!(actual, expected, "optimized={optimized}: {stderr}");
    }
}

#[test]
fn subaggregate_copies_use_the_selected_type_and_preserve_siblings() {
    sim_cli::run_case(
        SUITE,
        "subaggregate_types",
        "selected aggregate types passed\n",
        "",
        &[],
    );
}

#[test]
fn subarray_copies_follow_declaration_order_and_capture_nba_sources() {
    sim_cli::run_case(
        SUITE,
        "subarray_order",
        "subarray ordering passed\n",
        "",
        &[],
    );
}

#[test]
fn distinct_nominal_subaggregate_types_remain_illegal() {
    for optimized in [false, true] {
        let output =
            sim_cli::invoke_with_env(SUITE, "subaggregate_incompatible", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "incompatible types were accepted");
        assert!(output.stdout.is_empty(), "{output:?}");
        // Slang can diagnose before the owned lowerer; accept either layer's
        // type diagnostic, but not a missing fixture or a generated-C error.
        assert!(
            stderr.contains("incompatible") || stderr.contains("cannot be assigned to type"),
            "{stderr}"
        );
    }
}

#[test]
fn fixed_streaming_handles_exact_excess_and_multiple_segments() {
    sim_cli::run_case(
        SUITE,
        "stream_fixed_valid",
        "fixed streaming passed\n",
        "",
        &[],
    );
}

#[test]
fn insufficient_streams_fail_with_the_size_diagnostic() {
    for fixture in [
        "stream_short",
        "stream_multi_short",
        "stream_runtime_source_short",
    ] {
        for optimized in [false, true] {
            let output = sim_cli::invoke_with_env(SUITE, fixture, optimized, &[], &[], &[]);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{fixture}, optimized={optimized}");
            assert!(
                stderr.contains("streaming unpack source has insufficient bits"),
                "{fixture}: {stderr}"
            );
            assert!(output.stdout.is_empty(), "{fixture}: {output:?}");
        }
    }
}

#[test]
fn fixed_stream_bounds_error_still_writes_the_valid_portion() {
    for optimized in [false, true] {
        let output =
            sim_cli::invoke_with_env(SUITE, "stream_outside_bounds", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "bounds error must mark simulation failed"
        );
        assert!(stderr.contains("outside declared bounds"), "{stderr}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "in-range portion preserved\n"
        );
    }
}

#[test]
fn packed_directions() {
    sim_cli::run_case(
        SUITE,
        "packed_directions",
        "packed directions passed\n",
        "",
        &[],
    );
}

#[test]
fn packed_bounds() {
    sim_cli::run_case(SUITE, "packed_bounds", "packed bounds passed\n", "", &[]);
}

#[test]
fn packed_element_widths() {
    sim_cli::run_case(
        SUITE,
        "packed_element_widths",
        "packed element widths passed\n",
        "",
        &[],
    );
}

#[test]
fn packed_selection_capture() {
    sim_cli::run_case(
        SUITE,
        "packed_selection_capture",
        "packed selection capture passed\n",
        "",
        &[],
    );
}
