//! SYN-038 public-CLI witnesses for process declarations in an interface body.

use crate::sim_cli;

#[test]
fn interface_body_latch_and_ff_members_match_in_both_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = concat!(
        "latch open=3c\n",
        "ff issue q=00 captured=00\n",
        "latch held=3c\n",
        "ff commit q=5a captured=00\n",
        "ff issue q=5a captured=00\n",
        "ff commit q=c3 captured=5a\n",
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "interface_processes",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/interface_processes, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout.as_slice(),
            expected_stdout.as_bytes(),
            "{label}"
        );
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            "",
            "{label}"
        );
    }
}
