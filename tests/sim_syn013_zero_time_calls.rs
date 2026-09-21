//! SYN-013 finite zero-time subroutine, lifetime, and reference evidence.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn zero_time_calls_preserve_fixed_values_lifetimes_and_references() {
    sim_cli::run_case(
        "syn013_zero_time_calls",
        "zero_time_calls",
        "default=4,9,10 row=5,6,7 snapshot=4,6,7 forwarded=17 selected=7 workers=6,33\n",
        "",
        &[],
    );
}

#[test]
fn verilog_2001_zero_time_calls_preserve_automatic_activations() {
    sim_cli::run_case_with_args(
        "syn013_zero_time_calls",
        "legacy_calls",
        "legacy value=40 result=42 calls=2\n",
        "",
        &[],
        &["--edition", "2001"],
    );
}
