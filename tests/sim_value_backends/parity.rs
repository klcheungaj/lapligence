use super::{sim_cli, sim_harness};

#[test]
fn compact_parity_arithmetic_width() {
    sim_cli::run_case_backend_parity(
        "data_types_extended",
        "scalable_arithmetic",
        "PASS scalable_arithmetic WIDTH=2048\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_selections() {
    sim_cli::run_case_backend_parity(
        "group1_repairs",
        "packed_selection_capture",
        "packed selection capture passed\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_concat_streams() {
    sim_cli::run_case_backend_parity(
        "data_types_next",
        "syn_015_stream_width_matrix",
        "SYN015_STREAM_WIDTH_MATRIX_PASS\n",
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "data_types_next",
        "syn_015_runtime_streams",
        "SYN015_RUNTIME_STREAMS_PASS\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_nets_strength_force() {
    sim_cli::run_case_backend_parity(
        "net_resolution",
        "alias_force_and_strength",
        "CHECK: base=11\nCHECK: forced=00\nCHECK: released=11\n",
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "force",
        "Force_Selected_Net_Overlay",
        "forced=0010\nupper_release=1110\nall_release=1100\nconflict=xxxx\nz_release=0011\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_nba_capture() {
    sim_cli::run_case_backend_parity(
        "dynamic_ownership",
        "selected_nba_capture",
        "0\n42\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_rtl_001() {
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_001",
        "packed_boundaries",
        include_str!("../fixtures/sim/feature_completion/rtl_001/packed_boundaries.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_001",
        "nested_records",
        include_str!("../fixtures/sim/feature_completion/rtl_001/nested_records.out"),
        &[],
        &[],
    );
}

#[test]
fn compact_parity_rtl_002_large_arrays() {
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_002",
        "capacity_copy",
        "PASS rtl002 capacity copy\n",
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_002",
        "stream_services",
        "PASS rtl002 stream services\n",
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_002",
        "integration_views",
        "PASS rtl002 integration views\n",
        &[],
        &[],
    );
}

#[test]
fn compact_parity_rtl_003() {
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_003",
        "wide_port",
        include_str!("../fixtures/sim/feature_completion/rtl_003/wide_port.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_003",
        "selector_capture",
        include_str!("../fixtures/sim/feature_completion/rtl_003/selector_capture.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        "feature_completion/rtl_003",
        "ref_scan_selected",
        include_str!("../fixtures/sim/feature_completion/rtl_003/ref_scan_selected.out"),
        &[],
        &[],
    );
}

#[cfg(unix)]
#[test]
fn compact_parity_vpi_waveform() {
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = sim_harness::TempDir::new("parity-vpi").expect("plugin directory");
    let plugin = directory.path().join("plugin.so");
    let output = sim_harness::run_command(
        Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".to_owned()))
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared", "-I",
            ])
            .arg(root.join("src/sim/rt"))
            .arg(root.join("tests/fixtures/sim/vpi/vpi_partitioned_nets.c"))
            .arg("-o")
            .arg(&plugin),
        Duration::from_secs(60),
    )
    .expect("compile plugin");
    assert!(output.status.success(), "{output:?}");
    sim_cli::run_case_backend_parity(
        "waveform",
        "partitioned_nets",
        "vpi partitioned net shapes ok\n",
        &[],
        &[("LLG_VPI_PLUGIN", plugin.to_str().expect("plugin path"))],
    );
}
