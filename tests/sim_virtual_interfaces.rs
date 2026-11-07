//! End-to-end coverage for the bounded virtual-interface runtime subset.
//!
//! Checked-in fixtures exercise rebinding through a class, modport member and
//! imported-method restrictions, clocking-region reads, fixed virtual-interface
//! arrays, null-handle diagnostics, and nominal specialization errors. Every
//! executable case runs with and without optimizer passes.

use crate::sim_cli;

#[test]
fn virtual_interface_rebinding_and_views_match_across_optimizer_modes() {
    sim_cli::run_case(
        "virtual_interfaces",
        "rebind_class",
        concat!(
            "first=3 second=0 sample=3 value=3\n",
            "first=3 second=6 sample=6 value=6\n",
            "modport=8 array=6 sample=6 value=6\n",
        ),
        "INFO: rebind_class.sv:80:9: [SIM-001] (tb @ 6 ns) $finish called\n",
        &[],
    );
}

#[test]
fn modport_view_restrictions_preserve_permitted_accesses() {
    sim_cli::run_case(
        "virtual_interfaces",
        "modport_restriction",
        "input=1 output=0\n",
        "INFO: modport_restriction.sv:22:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}

#[test]
fn dynamic_virtual_interface_array_rebinds_elements() {
    sim_cli::run_case(
        "virtual_interfaces",
        "dynamic_array",
        "first=1 second=0 size=2\n",
        "INFO: dynamic_array.sv:21:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}

#[test]
fn queue_virtual_interface_array_rebinds_elements() {
    sim_cli::run_case(
        "virtual_interfaces",
        "queue",
        "first=1 second=0 size=2\n",
        "INFO: queue.sv:20:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}

#[test]
fn modport_input_write_is_rejected() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "modport_input_write",
        "cannot assign to input port 'input_data'",
    );
}

#[test]
fn modport_method_without_import_is_rejected() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "modport_method_not_imported",
        "cannot access 'set' via modport 'bus_if.monitor'",
    );
}

#[test]
fn null_virtual_interface_access_fails_at_runtime() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "null_access",
        "ERROR: [SIM-010] virtual interface access failed: virtual interface member access at tb:12:9",
    );
}

#[test]
fn incompatible_virtual_interface_specialization_is_rejected() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "parameter_mismatch",
        "cannot be assigned to type 'virtual interface bus_if#(W=4)'",
    );
}
