//! SYN-038 interface record field actual passed to an inout task formal.

use crate::sim_cli;

#[test]
fn interface_record_field_inout_task_preserves_neighbor_field_in_both_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "interface_record_inout",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/interface_record_inout, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.as_slice(), b"22 45\n", "{label}");
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            "",
            "{label}"
        );
    }
}
