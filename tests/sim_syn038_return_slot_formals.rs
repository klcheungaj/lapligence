//! SYN-038 static function return slots as output, inout, and ref actuals.

use crate::sim_cli;

#[test]
fn static_function_return_slots_bind_each_writable_formal_direction() {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );

    let expected_stdout = b"output=1 inout=1 ref=1\n";
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "syn038_pairwise",
            "return_slot_formals",
            optimized,
            &["--edition", "sv2009"],
            &[],
            &[],
        );
        let label = format!("syn038_pairwise/return_slot_formals, optimized={optimized}");
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
