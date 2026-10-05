use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/rtl_101";

#[test]
fn record_member_arrays_beyond_the_dense_threshold_are_columns() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/member_columns.out");
    sim_cli::run_case(SUITE, "member_columns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "member_columns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "member_columns", expected);
}

#[test]
fn records_beyond_the_packed_limit_move_as_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/oversized_values.out");
    sim_cli::run_case(SUITE, "oversized_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "oversized_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "oversized_values", expected);
}

#[test]
fn tagged_unions_beyond_the_packed_limit_move_as_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/tagged_columns.out");
    sim_cli::run_case(SUITE, "tagged_columns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "tagged_columns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "tagged_columns", expected);
}

#[test]
fn column_records_match_structure_patterns() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_101/record_patterns.out");
    sim_cli::run_case(SUITE, "record_patterns", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "record_patterns", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "record_patterns", expected);
}

#[test]
fn inactive_column_union_members_report_runtime_errors() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/rtl_101/tagged_inactive.sv");
    let location = sim_harness::source_display(&source);
    let expected_stderr = format!(
        "llg: runtime error: access to inactive tagged-union member w at {location}:10:23\n\
         llg: runtime error: access to inactive tagged-union member w at {location}:11:5\n"
    );
    sim_cli::run_case_checked_matrix(SUITE, "tagged_inactive", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        // The guarded read yields X and the guarded write leaves the active
        // member unchanged.
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "A x\nB 4\n",
            "{label}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    });
}

#[test]
fn generated_model_scales_with_the_declaration() {
    let expected = "5a 3 x\n";
    sim_cli::run_case(SUITE, "scale_65537", expected, "", &[]);
    sim_cli::run_case(SUITE, "scale_1048576", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "scale_1048576", expected, &[], &[]);
    for optimized in [false, true] {
        let mut sizes = Vec::new();
        for fixture in ["scale_65537", "scale_1048576"] {
            let directory = sim_harness::TempDir::new("rtl101-scale").expect("scratch");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "tests/fixtures/sim/feature_completion/rtl_101/{fixture}.sv"
            ));
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .arg(source)
                .args(["--top", "tb", "--gen-only", "--out-dir"]);
            command.arg(directory.path());
            if !optimized {
                command.arg("--no-opt");
            }
            let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
                .expect("generate column-record model");
            assert!(output.status.success(), "{output:?}");
            let model = std::fs::read_to_string(directory.path().join("sim/tb/model.c"))
                .expect("model source");
            sizes.push(model.len());
        }
        // A per-cell layout grows with the member extent; columns differ
        // only in the spelled bounds.
        assert!(sizes.iter().all(|size| *size < 100_000), "{sizes:?}");
        assert!(sizes[0].abs_diff(sizes[1]) < 256, "{sizes:?}");
    }
}

#[test]
fn neg_column_record_limits_and_single_writer() {
    sim_cli::reject_case(
        SUITE,
        "neg_whole_binding",
        "binding a whole value beyond packed capacity to a pattern variable is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_result_operand",
        "equality of a column-layout record value in `tb` requires record storage operands",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_member_write",
        "variable storage `tb.q.a` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_output_port_member_write",
        "variable storage `tb.q.w` has both a continuous assignment",
    );
}
