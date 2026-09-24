//! SYN-038 automatic fixed-record array initialization and selected `ref` use.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn automatic_record_array_initializer_and_selected_ref_match_oracle() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = b"call=c5 row0=11/a1 row1=c5/b2\ncall=e6 row0=11/a1 row1=e6/b2\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "record_auto_ref",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/record_auto_ref, optimized={optimized}");
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
