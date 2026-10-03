use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_010";

#[test]
fn aliased_positional_patterns_compete_with_independent_drivers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_010/alias_patterns.out");
    sim_cli::run_case(SUITE, "alias_patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "alias_patterns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "alias_patterns", expected);
}

#[test]
fn net_array_pattern_leaves_keep_cell_and_bit_contributions() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_010/net_array_patterns.out");
    sim_cli::run_case(SUITE, "net_array_patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "net_array_patterns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "net_array_patterns", expected);
}

#[test]
fn nested_fixed_targets_publish_every_leaf() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_010/nested_targets.out");
    sim_cli::run_case(SUITE, "nested_targets", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "nested_targets", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "nested_targets", expected);
    let witness = include_str!("../fixtures/sim/feature_completion/rtl_010/net_pattern_driver.out");
    sim_cli::run_case(
        SUITE,
        "net_pattern_driver",
        witness,
        "llg: $finish at time 1000 at tb:6:1\n",
        &[],
    );
}

#[test]
fn disjoint_variable_writers_share_storage() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_010/variable_writers.out");
    sim_cli::run_case(SUITE, "variable_writers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "variable_writers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "variable_writers", expected);
}

#[test]
fn time_zero_values_and_changed_leaf_notifications() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_010/time_zero_notifications.out");
    sim_cli::run_case(SUITE, "time_zero_notifications", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "time_zero_notifications", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "time_zero_notifications", expected);
}

#[test]
fn zero_time_feedback_converges_or_reports_nonconvergence() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_010/zero_time_feedback.out");
    sim_cli::run_case(SUITE, "zero_time_feedback", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "zero_time_feedback", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "zero_time_feedback", expected);
    sim_cli::reject_case(
        SUITE,
        "neg_feedback_nonconvergent",
        "nonconvergent zero-time execution",
    );
}

#[test]
fn descriptor_rows_scatter_without_flattening() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_010/descriptor_scatter.out");
    sim_cli::run_case(SUITE, "descriptor_scatter", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_scatter", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_scatter", expected);

    // Flattening a 65,537-cell row is over a megabit per leaf; descriptor
    // rows keep the generated model independent of the cell count.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl010-descriptor-scatter").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_010/descriptor_scatter.sv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .arg(source)
            .args(["--top", "tb", "--gen-only", "--out-dir"]);
        command.arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .expect("generate descriptor model");
        assert!(output.status.success(), "{output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(
            model.len() < 200_000,
            "unexpected model size: {}",
            model.len()
        );
    }
}

#[test]
fn parameterized_and_generated_composition() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_010/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "composition", expected);
}

#[test]
fn neg_overlapping_variable_writers() {
    sim_cli::reject_case(
        SUITE,
        "neg_member_two_continuous",
        "multiple continuous assignments to variable storage `tb.s.b`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_member_initializer",
        "variable storage `tb.s.a` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_packed_member_procedural",
        "has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_hierarchical_initializer",
        "variable storage `tb.c.v` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_cell_overlap",
        "has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_pattern_leaf_procedural",
        "variable storage `tb.arr[1]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_extra_writer",
        "has both a continuous assignment",
    );
}

#[test]
fn neg_runtime_pattern_leaf() {
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_pattern_leaf",
        "continuous assignment-pattern LHS in `tb` requires constant select indices",
    );
}

#[test]
fn delayed_pattern_driver_remains_assigned_to_adv_002() {
    sim_cli::reject_case(
        SUITE,
        "boundary_delayed_pattern",
        "delayed continuous assignment to a positional assignment-pattern LHS",
    );
}
