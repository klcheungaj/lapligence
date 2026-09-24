//! SYN-036 separates packed value capacity from fixed-array cell storage.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "syn036_capacity";

#[test]
fn direct_fixed_array_reduction_reads_cells_at_the_selected_limit() {
    sim_cli::run_case(
        SUITE,
        "cellwise_reduction",
        "PASS syn036 cellwise reduction\n",
        "",
        &[],
    );
}

#[test]
fn fixed_array_storage_rejects_one_cell_above_the_selected_limit() {
    sim_cli::reject_case(
        SUITE,
        "cell_limit",
        "fixed-array storage has 65537 cells; selected cell-wise storage limit is 65536 cells",
    );
}

#[test]
fn review_bundle_capacity_probe_rejects_the_65537th_cell() {
    sim_cli::reject_case_with_exact_stderr(
        "review_bundle",
        "r11_array_capacity_65537",
        "llg: codegen error: array `memory` in `tb`: fixed-array storage has 65537 cells; selected cell-wise storage limit is 65536 cells\n",
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_value_formals_reject_flattened_payloads_above_packed_capacity() {
    sim_cli::reject_case(
        SUITE,
        "aggregate_value_limit",
        "fixed formal shape fixed value payload is 1114112 bits; packed value capacity is 1048575 bits",
    );
}
