//! SIM-007: native aggregates, tagged values and pattern expressions. Fixed
//! arrays of strings, records with string/real/handle leaves and their
//! patterns, slices, conditionals, equality, calls and nonblocking writes,
//! in module storage and in procedural blocks.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_007";

#[test]
fn native_arrays_patterns_slices_calls_and_nbas() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_arrays.out");
    sim_cli::run_case_backend_parity(SUITE, "native_arrays", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_arrays", expected);
}

#[test]
fn record_handle_members_copy_identity_through_calls_and_arrays() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/handle_records.out");
    sim_cli::run_case_backend_parity(SUITE, "handle_records", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "handle_records", expected);
}

#[test]
fn native_tagged_unions_match_bind_and_check_members() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_tagged.out");
    sim_cli::run_case_backend_parity(SUITE, "native_tagged", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_tagged", expected);
}

/// Expected stderr with `{source}` standing for the fixture's absolute path,
/// which run-time tagged-access reports carry as their source location.
fn source_stderr(fixture: &str, template: &str) -> String {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    template.replace("{source}", &super::sim_harness::source_display(&source))
}

#[test]
fn inactive_native_tagged_members_report_and_store_nothing() {
    let expected = source_stderr(
        "native_tagged_inactive",
        "llg: runtime error: access to inactive tagged-union member S at {source}:10:28\n\
         llg: runtime error: access to inactive tagged-union member S at {source}:12:34\n\
         llg: runtime error: access to inactive tagged-union member F at {source}:12:39\n\
         llg: runtime error: access to inactive tagged-union member S at {source}:13:9\n\
         llg: runtime error: access to inactive tagged-union member I at {source}:17:9\n",
    );
    sim_cli::run_case_checked_matrix(SUITE, "native_tagged_inactive", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "1 []\n2 [] 0.0\n3 9\n4 kept\n",
            "{label}"
        );
        assert_eq!(stderr, expected, "{label}");
    });
}

#[test]
fn adopted_native_value_witnesses() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_tagged_native",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_tagged_native.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_binding",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_binding.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_array_pattern",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_array_pattern.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_record_slice",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_record_slice.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "witness_string_array_return",
        include_str!("../fixtures/sim/feature_completion/sim_007/witness_string_array_return.out"),
        &[],
        &[],
    );
}

#[test]
fn run_time_member_indices_select_native_record_elements() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/native_member_select.out");
    sim_cli::run_case_backend_parity(SUITE, "native_member_select", expected, &[], &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_select_limit", "xy []\n", &[], &[]);
    sim_cli::reject_case_with_args(
        SUITE,
        "bad_member_select_limit",
        "run-time index into a native record member array of 65 elements in `tb` is not supported; at most 64 are",
        &[],
    );
}

#[test]
fn record_elements_of_containers_compare_member_wise() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_element_equality.out");
    sim_cli::run_case_backend_parity(SUITE, "record_element_equality", expected, &[], &[]);
}

#[test]
fn native_patterns_keep_key_precedence_copies_and_source_order() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_patterns.out");
    sim_cli::run_case_backend_parity(SUITE, "native_patterns", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "bad_pattern_keys",
        "assignment pattern has multiple keys for index 0",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_pattern_keys",
        "assignment pattern has multiple keys for member 's'",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_pattern_shape",
        "assignment pattern for 'string$[2]' requires 2 elements but 3 were provided",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_pattern_shape",
        "value of type 'real' cannot be assigned to type 'string'",
    );
}

#[test]
fn record_container_members_behave_as_owned_containers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/record_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "record_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_containers", expected);
}

#[test]
fn record_container_members_wake_their_readers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_container_sensitivity.out");
    sim_cli::run_case_backend_parity(SUITE, "record_container_sensitivity", expected, &[], &[]);
}

#[test]
fn record_container_members_cross_subroutine_storage() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_container_calls.out");
    sim_cli::run_case_backend_parity(SUITE, "record_container_calls", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_container_calls", expected);
}

#[test]
fn record_container_members_cross_ports_and_wake_whole_record_readers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_container_ports.out");
    sim_cli::run_case_backend_parity(SUITE, "record_container_ports", expected, &[], &[]);
}

#[test]
fn discarded_queue_pops_remove_one_element() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/discarded_pops.out");
    sim_cli::run_case_backend_parity(SUITE, "discarded_pops", expected, &[], &[]);
}

#[test]
fn record_container_member_boundaries_are_rejected_explicitly() {
    sim_cli::reject_case(
        SUITE,
        "bad_record_container_element",
        "with a queue, dynamic or associative member is not supported here",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_record_container_nba",
        "nonblocking assignment of a record with a queue, dynamic or associative member `q` is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_record_assoc_equality",
        "equality of records with associative array member `k` is not supported",
    );
}

#[test]
fn nested_native_records_cross_calls_ports_and_selects() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/native_lifecycle.out");
    sim_cli::run_case_backend_parity(SUITE, "native_lifecycle", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_lifecycle", expected);
}

#[test]
fn nested_records_with_strings_reals_queues_and_handles_complete_their_lifecycle() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/native_lifecycle_containers.out");
    sim_cli::run_case_backend_parity(SUITE, "native_lifecycle_containers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_lifecycle_containers", expected);
}

#[test]
fn class_handle_stores_wake_ports_comb_logic_and_event_controls() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/handle_sensitivity.out");
    sim_cli::run_case_backend_parity(SUITE, "handle_sensitivity", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "bad_chandle_record_port",
        "is not a valid type for a port because it contains type 'chandle'",
    );
}

#[test]
fn procedural_block_records_own_their_members() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/block_records.out");
    sim_cli::run_case_backend_parity(SUITE, "block_records", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "block_records", expected);
}

#[test]
fn procedural_block_records_keep_static_and_automatic_lifetimes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/block_record_lifetimes.out");
    sim_cli::run_case_backend_parity(SUITE, "block_record_lifetimes", expected, &[], &[]);
    // The emptied associative member warns on each read of a missing key
    // (SV 7.8.6): its default does not survive into the next entry.
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("block_record_lifetimes.sv");
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "block_record_lifetimes",
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        "llg container warning: associative-array read of a nonexistent entry returns the default\n\
         llg container warning: associative-array read of a nonexistent entry returns the default\n",
    );
}

#[test]
fn procedural_block_record_boundaries_are_rejected_explicitly() {
    sim_cli::reject_case(
        SUITE,
        "bad_block_record_fork",
        "automatic record `r` in `tb` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_block_record_automatic_nba",
        "nonblocking assignment to automatic variable 'r' is not allowed",
    );
}

#[test]
fn procedural_block_strings_and_handles_behave_like_module_variables() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_007/block_natives.out");
    sim_cli::run_case_backend_parity(SUITE, "block_natives", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "block_natives", expected);
}

#[test]
fn procedural_block_strings_and_handles_keep_static_and_automatic_lifetimes() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/block_native_lifetimes.out");
    sim_cli::run_case_backend_parity(SUITE, "block_native_lifetimes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "block_native_lifetimes", expected);
}

#[test]
fn procedural_block_native_boundaries_are_rejected_explicitly() {
    sim_cli::reject_case(
        SUITE,
        "bad_block_native_fork",
        "automatic variable `name` in `tb` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "bad_block_native_always_fork",
        "automatic variable `h` in `tb` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported",
    );
}

#[test]
fn record_member_defaults_initialize_module_and_block_records() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_member_defaults.out");
    sim_cli::run_case_backend_parity(SUITE, "record_member_defaults", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_member_defaults", expected);
    sim_cli::reject_case(
        SUITE,
        "bad_record_member_default",
        "member default of `inner` is not supported: the value has no representation for this member type (record `m` in `tb`)",
    );
}

#[test]
fn module_record_initializers_accept_any_record_source() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_initializers.out");
    sim_cli::run_case_backend_parity(SUITE, "record_initializers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_initializers", expected);
}

#[test]
fn nested_records_and_member_arrays_compare_member_wise() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_007/record_view_equality.out");
    sim_cli::run_case_backend_parity(SUITE, "record_view_equality", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_view_equality", expected);
    sim_cli::reject_case(
        SUITE,
        "bad_record_view_equality",
        "equality of records with associative array member `sub__k` is not supported",
    );
}
