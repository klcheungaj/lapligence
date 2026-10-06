use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_099";

#[test]
fn descriptor_slices_on_output_ports_are_interval_writers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_099/descriptor_slice_writers.out");
    sim_cli::run_case(SUITE, "descriptor_slice_writers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_slice_writers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_slice_writers", expected);
}

#[test]
fn neg_overlapping_descriptor_slice_writers() {
    sim_cli::reject_case(
        SUITE,
        "neg_descriptor_slice_procedural",
        "variable storage `tb.a[0] through tb.a[3]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_descriptor_slice_ports",
        "multiple continuous assignments to variable storage `tb.a[0] through tb.a[3]`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_descriptor_rows_ff",
        "variable storage `tb.b[2][0] through tb.b[3][15]` has both a continuous assignment",
    );
}

#[test]
fn record_arrays_beyond_the_packed_limit_execute_as_values() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_099/record_array_values.out");
    sim_cli::run_case(SUITE, "record_array_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "record_array_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_array_values", expected);

    // A per-cell expansion of the 1,048,576-cell records would grow the model
    // with the extent; descriptor transport keeps it bounded.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl099-records").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_099/record_array_values.sv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .arg(&source)
            .args(["--top", "tb", "--gen-only", "--out-dir"]);
        command.arg(directory.path());
        // The fixture's deliberate lint errors are checked by `run_case`.
        if let Some(lint) = sim_cli::expected_lint(&source) {
            command.args(lint.allow_args());
        }
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
            .expect("generate record-array model");
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
