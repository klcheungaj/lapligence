//! Direct runtime regression source for callsite-specific VPI sizing.
#[path = "support/sim.rs"]
mod sim_harness;
use llg::sim;
use std::process::Command;
use std::time::Duration;
const SOURCE: &str = include_str!("fixtures/sim/review_batch3/vpi_callsite_sizes.c");

#[test]
fn sized_vpi_function_keeps_per_callsite_metadata() {
    check_sized_vpi_calls(false);
}

#[test]
fn sized_vpi_function_releases_owned_values_across_reinitialization() {
    check_sized_vpi_calls(true);
}

fn check_sized_vpi_calls(repeated: bool) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-callsite-sizes", |dir| {
        let executable = sim::build::build_model_cmake(dir, &[("vpi_callsite_sizes.c", SOURCE)])
            .map_err(|error| error.to_string())?;
        let mut command = Command::new(&executable);
        if repeated {
            command.arg("--repeat");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"vpi callsite sizes ok\n");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    })
    .expect("sized VPI function callsite regression");
}
