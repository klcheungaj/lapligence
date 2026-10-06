//! SYN-038 public-CLI evidence for packed-union interface storage and ports.

use crate::sim_cli;

#[test]
fn interface_union_field_nba_and_child_input_actual_match_in_both_modes() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = concat!(
        "first word=3a nibbles=3/a child=3a/3/a\n",
        "second word=c5 nibbles=c/5 child=c5/c/5\n",
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "union_interface",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/union_interface, optimized={optimized}");
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
        assert_eq!(output.stderr.as_slice(), b"", "{label}");
    }
}
