//! SYN-038 remaining legal storage/write-kind pairings.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

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
        source.display()
    );

    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "storage_write_remainders",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let label = format!("storage_write_remainders, optimized={optimized}");
        assert_eq!(output.status.code(), Some(0), "{label}");
        assert_eq!(output.stdout.as_slice(), expected_stdout, "{label}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    }
}
