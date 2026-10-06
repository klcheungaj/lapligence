use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_005";

#[test]
fn fixed_equality_operands_compare_immediate_members() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_005/equality_operands.out");
    sim_cli::run_case(SUITE, "equality_operands", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "equality_operands", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "equality_operands", expected);
}

#[test]
fn conditional_arms_keep_reached_effects_and_immediate_defaults() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_005/conditional_effects.out");
    sim_cli::run_case(SUITE, "conditional_effects", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "conditional_effects", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "conditional_effects", expected);
}

#[test]
fn dynamic_and_static_casts_share_one_success_contract() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_005/cast_contract.out");
    sim_cli::run_case(SUITE, "cast_contract", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "cast_contract", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "cast_contract", expected);
}

#[test]
fn task_form_cast_failure_reports_and_preserves_destination() {
    // Values are written to stderr so the public contract is one exact
    // stream: the failed cast reports its source location and continues.
    sim_cli::reject_case_with_exact_stderr(
        SUITE,
        "cast_task_failure",
        "A 5\nllg: runtime error: $cast failed to assign an incompatible value at tb:14:5\nB 5\n",
        &[],
    );
}

#[test]
fn descriptor_casts_and_conditionals_stay_unflattened() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected = include_str!("../fixtures/sim/feature_completion/rtl_005/descriptor_casts.out");
    sim_cli::run_case(SUITE, "descriptor_casts", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_casts", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_casts", expected);

    // Mid-size conditionals and reshaping casts once expanded every cell into
    // megabytes of C; descriptor operations keep the model independent of
    // the 65,537- and 5,000-cell extents.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl005-descriptor-shape").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_005/descriptor_casts.sv");
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
        assert!(model.contains("llg_fixed_array_compare"));
        assert!(model.contains("llg_fixed_array_merge"));
        assert!(
            model.len() < 200_000,
            "unexpected model size: {}",
            model.len()
        );
    }
}

#[test]
fn z_conditional_policy_witness() {
    // Adopted FND-002 witness: Table 11-20 maps Z/Z to X (owner policy).
    sim_cli::run_case_with_args(
        SUITE,
        "z_conditional.v",
        "x\n",
        "llg: $finish at time 0 at tb:7:1\n",
        &[],
        &["--edition", "v2001"],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "z_conditional.v",
        "x\n",
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn neg_bad_bitstream_size() {
    sim_cli::reject_case(
        SUITE,
        "neg_bad_bitstream_size",
        "cannot be converted to type 'A'",
    );
}

#[test]
fn neg_unpacked_cast_size() {
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_cast_size",
        "cannot be converted to type 'l3_t'",
    );
}

#[test]
fn neg_cast_aggregate_dest() {
    sim_cli::reject_case(
        SUITE,
        "neg_cast_aggregate_dest",
        "$cast destination must be a singular value",
    );
}

#[test]
fn neg_nominal_equality() {
    sim_cli::reject_case(
        SUITE,
        "neg_nominal_equality",
        "invalid operands to binary expression",
    );
}

#[test]
fn neg_shape_equality() {
    sim_cli::reject_case(
        SUITE,
        "neg_shape_equality",
        "invalid operands to binary expression",
    );
}
