//! SIM-012: virtual-interface methods and dynamic receivers.
//! Oracles are derived by hand in the fixture readme.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_012";

#[test]
fn timed_interface_tasks_dispatch_through_every_receiver_storage() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_012/timed_dispatch.out");
    sim_cli::run_case_backend_parity(SUITE, "timed_dispatch", expected, &[], &[]);
}

#[test]
fn event_formals_keep_identity_through_interface_dispatch() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_012/event_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "event_formals", expected, &[], &[]);
}

fn null_interface_check(stdout: &'static str, site: &'static str) -> impl Fn(&str, &Output) {
    move |label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        let runtime = stderr
            .lines()
            .filter(|line| !line.starts_with("Warning: "))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        assert_eq!(
            runtime,
            format!(
                "llg: virtual interface access failed: {site}\nllg: severity fatal: {site}: virtual interface access failed\n"
            ),
            "{label}"
        );
    }
}

#[test]
fn null_interface_calls_and_accesses_fail_at_their_source_site() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "null_call",
        &[],
        &null_interface_check("a.x=1 b.x=0\n", "virtual interface method call at tb:20:5"),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "null_member",
        &[],
        &null_interface_check("x=3\n", "virtual interface member access at tb:15:5"),
    );
}

#[test]
fn modport_and_specialization_violations_are_frontend_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_modport_write",
        "cannot assign to input port 'x'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_specialization",
        "value of type 'interface ifc#(W=8)' cannot be assigned to type 'virtual interface ifc#(W=4)'",
    );
}
