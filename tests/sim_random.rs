//! End-to-end tests for Verilog-2001 §17.9 / SystemVerilog §20.15.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const EXPECTED: &str = "implicit=303379748,-1064739199\n\
random=-2147414528,-1671855048,1129920902 seed=-1017563188\n\
uniform=-2,-2,1 seed=-1017563188\n\
normal=10,9 seed=-977101388\n\
exponential=44,11 seed=460696424\n\
poisson=1,13 seed=849187386\n\
chi=2,2 seed=-351915328\n\
t=1,1 seed=849187386\n\
erlang=55,3 seed=-291802762\n";

#[test]
fn legacy_random_distributions_match_annex_n_in_both_optimizer_modes() {
    sim_cli::run_case("random", "basic", EXPECTED, "", &[]);
}

#[test]
fn legacy_random_distributions_match_verilog_2001_policy() {
    sim_cli::run_case_with_args("random", "basic", EXPECTED, "", &[], &["--edition", "2001"]);
}
