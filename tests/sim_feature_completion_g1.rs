//! Group 1 feature-completion slice: nested fixed-array element selections,
//! expression sequencing, and fixed bit-stream forms. Fixtures run through the
//! public simulator in both optimizer modes with independent stdout oracles.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn port_label_actual_scope() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_label_actual_scope",
        "p=1 c=1 r=1\np=0 c=0 r=0\n",
        "",
        &[],
    );
}

#[test]
fn port_array_aggregate_roundtrip() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_array_aggregate_roundtrip",
        "y0=-6 y1=-101\ny0=300 y1=27\n",
        "",
        &[],
    );
}

#[test]
fn port_array_element_actual_roundtrip() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_array_element_actual",
        "d=11 22\nd=31 22\n",
        "",
        &[],
    );
}

#[test]
fn port_array_nonconstant_actual_is_rejected() {
    sim_cli::reject_case(
        "feature_completion/g1_22",
        "port_array_nonconstant_actual",
        "non-constant element actual",
    );
}

#[test]
fn port_inout_not_two_copies() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_inout_not_two_copies",
        "none=z\nparent=1\nchild=0\nconflict=x\nrelease=z\n",
        "",
        &[],
    );
}

// ── G1-23 interface and modport usage ─────────────────────────────────────

#[test]
fn interface_shared_modports() {
    sim_cli::run_case(
        "feature_completion/g1_23",
        "interface_shared_modports",
        "d=2a dd=2b bus=2a\nd=40 dd=41 bus=40\n",
        "",
        &[],
    );
}

#[test]
fn interface_array_parameter() {
    sim_cli::run_case(
        "feature_completion/g1_23",
        "interface_array_parameter",
        "got=a0 a1\n",
        "",
        &[],
    );
}

#[test]
fn modport_access_error() {
    sim_cli::reject_case(
        "feature_completion/g1_23",
        "modport_access_error",
        "cannot assign to input port",
    );
}

#[test]
fn modport_access_error_nested() {
    sim_cli::reject_case(
        "feature_completion/g1_23",
        "modport_access_error_nested",
        "cannot assign to input port",
    );
}
