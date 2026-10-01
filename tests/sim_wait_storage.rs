#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn waits_rearm_and_disable_in_both_optimizer_modes() {
    sim_cli::run_case("wait_storage", "rearm", "4 4 4 4 1\n", "", &[]);
}

#[test]
fn narrow_and_wide_waits_preserve_x_and_z_edges() {
    sim_cli::run_case("wait_storage", "four_state", "3 3 4 4\n", "", &[]);
}
