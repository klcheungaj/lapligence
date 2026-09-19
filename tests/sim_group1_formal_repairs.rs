//! R09/R14 regressions through the public CLI, in both optimizer modes.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn packed_callback_activations() {
    sim_cli::run_case("group1_repairs", "packed_callback_activations", "packed callbacks passed\n", "", &[]);
}

#[test]
fn packed_formal_recursion() {
    sim_cli::run_case("group1_repairs", "packed_formal_recursion", "packed recursion passed\n", "", &[]);
}

#[test]
fn packed_reference_activations() {
    sim_cli::run_case("group1_repairs", "packed_reference_activations", "packed references passed\n", "", &[]);
}

#[test]
fn packed_formal_member_views() {
    sim_cli::run_case("group1_repairs", "packed_formal_member_views", "packed member views passed\n", "", &[]);
}

#[test]
fn packed_formal_copyout_capture() {
    sim_cli::run_case("group1_repairs", "packed_formal_copyout_capture", "packed copyout capture passed\n", "", &[]);
}

#[test]
fn packed_const_reference_remains_read_only() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env("group1_repairs", "packed_const_reference_rejected", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success() && output.stdout.is_empty(), "{output:?}");
        assert!(stderr.contains("cannot assign to read-only variable") || stderr.contains("cannot write through const ref"), "{stderr}");
    }
}

#[test]
fn packed_reference_members_do_not_bypass_nba_lifetime_checks() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env("group1_repairs", "packed_reference_nba_rejected", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success() && output.stdout.is_empty(), "{output:?}");
        assert!(stderr.contains("nonblocking assignment to automatic")
            || stderr.contains("targets stack-backed input/formal/local storage")
            || stderr.contains("nonblocking writes through reference formals"), "{stderr}");
    }
}
