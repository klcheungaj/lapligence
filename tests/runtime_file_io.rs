//! Direct runtime checks for descriptor masks, ordinary-file ownership, and
//! invalid/closed descriptor status.

#[path = "support/sim.rs"]
mod sim_harness;

use std::fs;

use llg::sim;

const FILE_PROBE: &str = include_str!("runtime_value_storage/file_output_isolation_probe.c");

const FILE_INPUT_PROBE: &str = include_str!("runtime_value_storage/file_input_isolation_probe.c");

#[test]
fn descriptor_masks_and_boundaries_are_portable() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("runtime-file-io", |dir| {
        fs::write(dir.join("runtime_file_io_probe.c"), FILE_PROBE)
            .map_err(|error| error.to_string())?;
        let executable = sim::build::build_model_cmake(
            dir,
            &[
                ("runtime_file_io_probe.c", FILE_PROBE),
                (
                    "test_value_temporaries.h",
                    include_str!("runtime_value_storage/test_value_temporaries.h"),
                ),
            ],
        )
        .map_err(|error| error.to_string())?;
        let output = sim_harness::run_executable_output(&executable)?;
        assert_eq!(output.stdout, b"probe=7\n");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    })
    .expect("runtime file descriptor probe");
}

#[test]
fn formatted_character_line_and_binary_input_are_portable() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("runtime-file-input", |dir| {
        let executable = sim::build::build_model_cmake(
            dir,
            &[
                ("runtime_file_input_probe.c", FILE_INPUT_PROBE),
                (
                    "test_value_temporaries.h",
                    include_str!("runtime_value_storage/test_value_temporaries.h"),
                ),
            ],
        )
        .map_err(|error| error.to_string())?;
        let output = sim_harness::run_executable_output(&executable)?;
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    })
    .expect("runtime file input probe");
}
