//! SYN-039 final selected-profile acceptance through the public simulator CLI.
//!
//! The fixtures intentionally compose completed Core, Extended and capacity
//! tracks. Every CLI helper runs the source once with optimization and once
//! with `--no-opt`; exact stdout and the selected rejection diagnostic are
//! checked independently of the generated model's implementation details.

use crate::sim_cli;

const SUITE: &str = "syn039_acceptance";

#[test]
fn array_of_records_cross_ports_functions_comb_and_ff() {
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "array_record_datapath",
        "records=13/19 total=20\n",
        "",
        &["+seed=1"],
    );
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "array_record_datapath",
        "records=15/21 total=22\n",
        "",
        &["+seed=2"],
    );
}

#[test]
fn parameterized_interface_generate_and_memory_blocks_compose() {
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "interface_generate_memory",
        "first=10/22 bus=10/22\nsecond=40/32 bus=40/32\n",
        "",
        &["+address=0"],
    );
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "interface_generate_memory",
        "first=20/32 bus=20/32\nsecond=40/32 bus=40/32\n",
        "",
        &["+address=1"],
    );
}

#[test]
fn aliased_wired_multi_instance_network_preserves_resolution() {
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "alias_wired_multi",
        "start=z/z\nzero=0/0\none=1/1\nboth=0/0\n",
        "",
        &["+start=0"],
    );
    sim_cli::run_case_with_runtime_args(
        SUITE,
        "alias_wired_multi",
        "start=0/0\nzero=0/0\none=1/1\nboth=0/0\n",
        "",
        &["+start=1"],
    );
}

#[test]
fn selected_pattern_udp_and_configuration_paths_compose() {
    let map = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("root.map");
    let map = map.to_string_lossy();
    let args = [
        "--edition",
        "2009",
        "--top",
        "syn039_select:config",
        "--libmap",
        map.as_ref(),
    ];
    sim_cli::run_case_with_source_prefix_and_runtime_args(
        SUITE,
        "extended_top",
        &["extended_config"],
        "configured=7a pattern=5a bound=5a\n",
        "",
        &args,
        &["+left=5a"],
    );
    sim_cli::run_case_with_source_prefix_and_runtime_args(
        SUITE,
        "extended_top",
        &["extended_config"],
        "configured=23 pattern=01 bound=01\n",
        "",
        &args,
        &["+left=03"],
    );
}

#[test]
fn sequential_udp_remains_a_single_fault_profile_rejection() {
    sim_cli::reject_case(
        SUITE,
        "unsupported_sequential_udp",
        "user-defined primitive instance is not supported",
    );
}
