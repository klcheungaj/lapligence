//! SYN-038 module runtime initialization from an interface data member.

use crate::sim_harness;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

const FIXTURE_SOURCE: &str =
    include_str!("fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv");
const EXPECTED_STDOUT: &str = "copy=00,source=5a,control=00\n";

#[test]
fn module_initializer_reads_bound_interface_storage_in_both_modes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    assert!(FIXTURE_SOURCE.starts_with(
        "// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv\n"
    ));
    for anchor in [
        "bit [7:0] value;",
        "bit [7:0] local_seed;",
        "logic [7:0] same_scope_copy = local_seed;",
        "logic [7:0] copy = bus.value;",
        "copy !== 8'h00",
        "local_seed = 8'h34;",
        "bus.value = 8'h5a;",
        "same_scope_copy !== 8'h00",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(anchor),
            "fixture lost anchor: {anchor}"
        );
    }
    for assertion in [
        "module declaration initializer source mismatch",
        "module declaration initializer did not capture source value",
    ] {
        assert!(
            FIXTURE_SOURCE.contains(assertion),
            "fixture lost oracle: {assertion}"
        );
    }

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv");
    let control_line = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            line.contains("same_scope_copy = local_seed;")
                .then_some((index + 1, line))
        })
        .expect("fixture contains the same-scope initializer control");
    let control_column = control_line
        .1
        .find("local_seed")
        .expect("same-scope initializer names its source")
        + 1;
    let warning_line = FIXTURE_SOURCE
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            line.contains("logic [7:0] copy = bus.value;")
                .then_some((index + 1, line))
        })
        .expect("fixture contains the interface runtime initializer");
    let warning_column = warning_line
        .1
        .find("bus.value")
        .expect("initializer contains its bound interface member")
        + 1;
    let expected_stderr = format!(
        "Warning: {}:{}:{} initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\n\
Warning: {}:{}:{} initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\n",
        sim_harness::source_display(&fixture_path), control_line.0, control_column,
        sim_harness::source_display(&fixture_path), warning_line.0, warning_column
    );

    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("interface-runtime-initializer")
            .expect("isolated CLI directory");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command.current_dir(directory.path()).args(["--top", "tb"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.args(["--edition", "sv2009"]).arg(&fixture_path);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .unwrap_or_else(|error| panic!("optimized={optimized}: {error}"));
        assert!(
            output.status.success(),
            "optimized={optimized}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            EXPECTED_STDOUT,
            "optimized={optimized}"
        );
        assert_eq!(
            crate::sim_harness::strip_lint_reports(&output.stderr),
            expected_stderr,
            "optimized={optimized}"
        );
    }
}
