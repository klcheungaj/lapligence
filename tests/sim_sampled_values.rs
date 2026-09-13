//! H21 sampled-value system-function acceptance tests.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn sampled_value_domains_preserve_history_and_preponed_values() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sampled_values",
        "SAMPLED 0000 0 xxxx 0\nSAMPLED 0001 1 xxxx 0\nSAMPLED 0000 0 0000 0\n",
        "",
        &[],
    );
}

#[test]
fn global_clock_sampled_values_use_the_declared_domain() {
    sim_cli::run_case(
        "concurrent_assertions",
        "global_sampled_values",
        "GLOBAL xx 0\nGLOBAL 00 1\n",
        "",
        &[],
    );
}

#[test]
fn sampled_status_functions_use_the_lsb_and_preserve_xz_edges() {
    sim_cli::run_case(
        "concurrent_assertions",
        "sampled_status_edges",
        "EDGE 0 1 0 1\nEDGE 1 0 0 1\nEDGE 0 1 0 1\nEDGE 0 0 0 1\nEDGE 1 0 0 1\n",
        "",
        &[],
    );
}

#[test]
fn sampled_functions_in_properties_use_the_property_clock() {
    sim_cli::run_case(
        "concurrent_assertions",
        "assert_sampled_values",
        "ASSERT_SAMPLED PASS 1\nASSERT_SAMPLED PASS 0\n",
        "",
        &[],
    );
}

#[test]
fn sampled_functions_use_the_default_clocking_block() {
    sim_cli::run_case(
        "concurrent_assertions",
        "default_sampled_values",
        "DEFAULT x 1\nDEFAULT 1 0\n",
        "",
        &[],
    );
}

#[test]
fn sampled_functions_use_a_single_process_edge_clock() {
    sim_cli::run_case(
        "concurrent_assertions",
        "inferred_sampled_values",
        "INFERRED x 1\nINFERRED 1 0\n",
        "",
        &[],
    );
}

#[test]
fn unsupported_future_global_functions_fail_closed() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_future_global",
        "future global sampled-value function",
    );
}

#[test]
fn unsupported_sequence_status_fails_closed() {
    sim_cli::reject_case(
        "concurrent_assertions",
        "unsupported_sequence_status",
        "sequence `.triggered` status is not supported",
    );
}
