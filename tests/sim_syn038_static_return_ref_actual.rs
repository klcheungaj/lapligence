//! SYN-038 hierarchical static function return slots as ref actuals.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn hierarchical_static_function_return_slot_binds_to_task_ref_formal() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stderr = "llg: $finish at time 0 at tb:18:9\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "static_return_ref_actual",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let label = format!("static_return_ref_actual, optimized={optimized}");
        assert_eq!(output.status.code(), Some(0), "{label}");
        assert_eq!(
            output.stdout.as_slice(),
            b"static-return-ref=passed\n",
            "{label}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected_stderr,
            "{label}"
        );
    }
}
