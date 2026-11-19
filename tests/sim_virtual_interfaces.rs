//! End-to-end coverage for the bounded virtual-interface runtime subset.
//!
//! Checked-in fixtures exercise rebinding through a class, modport member and
//! imported-method restrictions, clocking-region reads, fixed virtual-interface
//! arrays, null-handle diagnostics, nominal specialization errors, and
//! interface instances nested in an interface selected through a handle
//! (values, deeper paths, instance array elements, modport views,
//! specializations and null handles). Every executable case runs with and
//! without optimizer passes.

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

#[test]
fn nested_interface_instance_is_a_virtual_interface_value() {
    sim_cli::run_case(
        "virtual_interfaces",
        "nested_value",
        concat!(
            "5 rx=0 tx_mon=x\n",
            "15 rx=1 tx_mon=1\n",
            "25 rx=1 tx_mon=0\n",
            "same=1 rx_vs_mon=0 tx=1\n",
        ),
        "INFO: nested_value.sv:59:5: [SIM-001] (tb @ 30 ns) $finish called\n",
        &[],
    );
}

#[test]
fn nested_interface_paths_follow_the_rebound_handle() {
    sim_cli::run_case(
        "virtual_interfaces",
        "nested_depth",
        concat!(
            "o1 xx xx xx xx\n",
            "o2 31 22 23 24\n",
            "hits 1 2 0\n",
            "o1 51 42 43 44\n",
            "eq 1 0 1\n",
        ),
        "INFO: nested_depth.sv:56:5: [SIM-001] (tb @ 2 ns) $finish called\n",
        &[],
    );
}

#[test]
fn nested_instance_array_elements_and_modport_views() {
    sim_cli::run_case(
        "virtual_interfaces",
        "nested_array_modport",
        "1 2 3 a b 5 9\n4 w=6\n",
        "INFO: nested_array_modport.sv:48:5: [SIM-001] (tb @ 4 ns) $finish called\n",
        &[],
    );
}

#[test]
fn nested_instances_of_parameterized_interfaces() {
    sim_cli::run_case(
        "virtual_interfaces",
        "nested_param",
        "f 12 34 5678 16\n",
        "INFO: nested_param.sv:27:5: [SIM-001] (tb @ 1 ns) $finish called\n",
        &[],
    );
}

#[test]
fn nested_instance_of_a_null_handle_fails_at_runtime() {
    for fixture in ["nested_null", "nested_unbound_type"] {
        let line = if fixture == "nested_null" { 19 } else { 20 };
        sim_cli::run_case_failing_at_runtime(
            "virtual_interfaces",
            fixture,
            &[],
            "before\n",
            &format!(
                "ERROR: [SIM-010] virtual interface access failed: virtual interface nested \
                 instance `l` at tb:{line}:12"
            ),
        );
    }
}

#[test]
fn generate_block_member_through_virtual_interface_is_rejected() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "nested_generate_rejected",
        "nested_generate_rejected.sv:16:5 generate block `g` of an interface is not supported \
         through a virtual interface",
    );
}

#[test]
fn wait_through_nested_instance_follows_rebinding() {
    sim_cli::run_case(
        "virtual_interfaces",
        "nested_wait_rebind",
        "2 woke w=2\n",
        "INFO: nested_wait_rebind.sv:34:9: [SIM-001] (tb @ 12 ns) $finish called\n",
        &[],
    );
}
