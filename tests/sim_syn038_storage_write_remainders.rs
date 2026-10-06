//! SYN-038 remaining legal storage/write-kind pairings.

use crate::sim_cli;
use crate::sim_harness;

#[test]
fn storage_write_remainder_cells_run_in_both_optimizer_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv");
    let expected_stdout = b"storage_write_remainders=passed\n";
    let expected_stderr = format!(
        "Warning: {}:101:33 non-void function 'hier_result' does not return a value\n\
         llg: $finish at time 4000 at tb:153:9\n",
        sim_harness::source_display(&source)
    );

    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "storage_write_remainders",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("storage_write_remainders, optimized={optimized}");
        assert_eq!(output.status.code(), Some(0), "{label}");
        assert_eq!(output.stdout.as_slice(), expected_stdout, "{label}");
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            expected_stderr,
            "{label}"
        );
    }
}
