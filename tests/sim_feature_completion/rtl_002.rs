use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_002";

#[test]
fn required_capacity_lane_executes_large_value_copy() {
    sim_cli::run_case(
        SUITE,
        "capacity_copy",
        "PASS rtl002 capacity copy\n",
        "",
        &[],
    );
}

#[test]
fn storage_boundaries_and_multidimensional_minimum_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "storage_capacity.v",
            "PASS rtl002 storage\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn dimension_product_overflow_rejects_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(SUITE, "overflow.v", "overflows", &["--edition", edition]);
    }
}

#[test]
fn capacity_copy_survives_owned_source_destruction() {
    sim_cli::run_case_after_db_drop(SUITE, "capacity_copy", "PASS rtl002 capacity copy\n");
}

#[test]
fn packed_capacity_remains_separate_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "packed_at.v",
            "PASS syn036 packed at\n",
            "",
            &[],
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(SUITE, "packed_above.v", "1048576", &["--edition", edition]);
        sim_cli::reject_case_with_args(
            SUITE,
            "above_capacity.v",
            "16777217 cells",
            &["--edition", edition],
        );
    }
}

#[test]
fn whole_memory_values_remain_systemverilog_only() {
    sim_cli::reject_case_with_args(
        SUITE,
        "whole_memory_illegal.v",
        "whole unpacked array value",
        &["--edition", "2001"],
    );
}

#[test]
fn adopted_automatic_and_reference_nba_negatives() {
    sim_cli::reject_case(SUITE, "neg_automatic_nba", "nonblocking");
    sim_cli::reject_case(SUITE, "neg_ref_nba", "nonblocking");
}

#[test]
fn nonflattened_inputs_locals_and_owned_return() {
    sim_cli::run_case(SUITE, "value_call", "PASS rtl002 value call\n", "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "value_call", "PASS rtl002 value call\n");
}

#[test]
fn descriptor_streams_and_memory_services() {
    sim_cli::run_case(
        SUITE,
        "stream_services",
        "PASS rtl002 stream services\n",
        "",
        &[],
    );
}

#[test]
fn selected_and_dynamic_dependencies_keep_cell_identity() {
    sim_cli::run_case(SUITE, "dependencies", "PASS rtl002 dependencies\n", "", &[]);
}

#[test]
fn generated_source_does_not_scale_with_logical_extent() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;
    let mut sizes = Vec::new();
    for count in [65537, 16777216] {
        let directory = sim_harness::TempDir::new("rtl002-source-shape").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_002/source_shape.v");
        let output = sim_harness::run_command(
            Command::new(env!("CARGO_BIN_EXE_llg"))
                .arg(source)
                .args(["--gen-only", "-D"])
                .arg(format!("RTL002_COUNT={count}"))
                .arg("--out-dir")
                .arg(directory.path()),
            Duration::from_secs(60),
        )
        .expect("generate capacity model");
        assert!(output.status.success(), "{output:?}");
        let source =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(source.contains("llg_fixed_array_stream_copy"));
        assert!(
            source.len() < 100_000,
            "unexpected model size: {}",
            source.len()
        );
        sizes.push(source.len());
    }
    assert!(sizes[0].abs_diff(sizes[1]) < 1024, "{sizes:?}");
    sim_cli::run_case(
        SUITE,
        "source_shape.v",
        "PASS rtl002 source shape\n",
        "",
        &[],
    );
}

#[test]
fn uniform_default_patterns_use_one_snapshot() {
    sim_cli::run_case(
        SUITE,
        "default_pattern",
        "PASS rtl002 default pattern\n",
        "",
        &[],
    );
}

#[test]
fn descriptor_waveform_registration_preserves_escaped_names() {
    sim_cli::run_case(SUITE, "waveform", "PASS rtl002 waveform\n", "", &[]);
}

#[test]
fn delayed_selected_cells_keep_independent_site_handles() {
    sim_cli::run_case(SUITE, "inertial", "PASS rtl002 inertial\n", "", &[]);
}

#[test]
fn descriptor_selected_refs_and_partitioned_nets_compose() {
    sim_cli::run_case(
        SUITE,
        "integration_views",
        "PASS rtl002 integration views\n",
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "integration_views",
        "PASS rtl002 integration views\n",
    );
}
