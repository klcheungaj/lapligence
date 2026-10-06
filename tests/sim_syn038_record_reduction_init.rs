//! SYN-038 fixed-record reduction in an automatic local initializer.

use crate::sim_cli;

#[test]
fn fixed_record_reduction_initializes_automatic_local() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = b"total=10\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "record_reduction_init",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/record_reduction_init, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.as_slice(), expected_stdout, "{label}");
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            "",
            "{label}"
        );
    }
}
