//! SYN-038 selected fixed-array procedural contexts through the public CLI.
//!
//! The checked-in fixture combines a function result assigned to a selected
//! `always_comb` element with issue-time selected-element and record NBAs in
//! `always_ff`. The Rust oracle is independent of the HDL checks and runs in
//! both optimizer modes under IEEE 1800-2009.

use crate::sim_cli;

#[test]
fn selected_array_process_targets_capture_expected_values() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = b"comb=5a,c3\nff=5a,44 records=d3/1,66/0\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "array_processes",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/array_processes, optimized={optimized}");
        assert!(
            output.status.success(),
            "{label}: status={:?}, stdout={}, stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.as_slice(), expected_stdout, "{label}");
        assert_eq!(output.stderr.as_slice(), b"", "{label}");
    }
}
