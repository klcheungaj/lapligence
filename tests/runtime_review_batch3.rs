//! Direct runtime regression source for callsite-specific VPI sizing.
//! Not executed during preparation of the static-review patch.
#[path = "support/sim.rs"]
mod sim_harness;
use llg::sim;
const SOURCE: &str = include_str!("fixtures/sim/review_batch3/vpi_callsite_sizes.c");

#[test]
fn sized_vpi_function_keeps_per_callsite_metadata() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-callsite-sizes", |dir| {
        let executable = sim::build::build_model_cmake(dir, &[("vpi_callsite_sizes.c", SOURCE)])
            .map_err(|error| error.to_string())?;
        let output = sim_harness::run_executable_output(&executable)?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, b"vpi callsite sizes ok\n");
        assert!(output.stderr.is_empty(), "{output:?}");
        Ok(())
    }).expect("sized VPI function callsite regression");
}
