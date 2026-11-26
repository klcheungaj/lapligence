//! End-to-end coverage for the bounded virtual-interface runtime subset.
//!
//! Checked-in fixtures exercise rebinding through a class, modport member and
//! imported-method restrictions, clocking-region reads, fixed virtual-interface
//! arrays, null-handle diagnostics, nominal specialization errors, and
//! interface instances nested in an interface selected through a handle
//! (values, deeper paths, instance array elements, modport views,
//! specializations and null handles), and unpacked members named through a
//! handle (values, targets, methods, waits and nonblocking writes). Every
//! executable case runs with and without optimizer passes.

use crate::{sim_cli, sim_harness};

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
fn string_member_leaves_the_other_members_reachable() {
    sim_cli::run_case(
        "virtual_interfaces",
        "string_member",
        "data=5 name=bus\n",
        "INFO: string_member.sv:19:9: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
    sim_cli::reject_case(
        "virtual_interfaces",
        "neg_string_member",
        "neg_string_member.sv:15:9 member `name` of virtual interface `bus_if` is a string or a handle",
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
        "null_access.sv:12:9 virtual interface access failed: virtual interface member access",
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
                "{fixture}.sv:{line}:12 virtual interface access failed: virtual interface nested \
                 instance `l`"
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

#[test]
fn named_events_through_virtual_interfaces() {
    sim_cli::run_case_backend_parity(
        "virtual_interfaces",
        "events",
        include_str!("fixtures/sim/virtual_interfaces/events.out"),
        &[],
        &[],
    );
}

#[test]
fn delayed_nonblocking_writes_go_to_the_instance_bound_at_issue() {
    sim_cli::run_case_backend_parity(
        "virtual_interfaces",
        "delayed_nba",
        include_str!("fixtures/sim/virtual_interfaces/delayed_nba.out"),
        &[],
        &[],
    );
}

#[test]
fn arrays_of_interface_instances_are_arrays_of_handles() {
    sim_cli::run_case_backend_parity(
        "virtual_interfaces",
        "instance_arrays",
        include_str!("fixtures/sim/virtual_interfaces/instance_arrays.out"),
        &[],
        &[],
    );
}

/// A run that ends in the fatal report of a null handle `site`.
fn ends_at_null_handle(fixture: &str, stdout: &str, site: &str) {
    sim_cli::run_case_checked_matrix("virtual_interfaces", fixture, &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        assert!(
            sim_harness::mentions(&String::from_utf8_lossy(&output.stderr), site),
            "{label}: {output:?}"
        );
    });
}

#[test]
fn named_event_through_a_null_handle_stops_at_once() {
    ends_at_null_handle(
        "null_event",
        "start\n",
        "null_event.sv:11:10 virtual interface access failed: virtual interface event",
    );
}

#[test]
fn event_controlled_update_through_a_null_handle_stops_at_the_event() {
    ends_at_null_handle(
        "null_event_nba",
        "issued\n",
        "null_event_nba.sv:11:5 virtual interface access failed: virtual interface member access",
    );
}

#[test]
fn event_arrays_and_assignments_go_to_the_bound_instance() {
    sim_cli::run_case_backend_parity(
        "virtual_interfaces",
        "event_members",
        include_str!("fixtures/sim/virtual_interfaces/event_members.out"),
        &[],
        &[],
    );
}

#[test]
fn event_array_element_through_a_null_handle_stops_at_once() {
    ends_at_null_handle(
        "null_event_array",
        "",
        "null_event_array.sv:10:8 virtual interface access failed: virtual interface event",
    );
}

#[test]
fn event_array_element_as_a_reference_actual_is_located() {
    sim_cli::reject_case(
        "virtual_interfaces",
        "neg_event_array_actual",
        "neg_event_array_actual.sv:11:18: [CODEGEN-002] an element of an event array of a class object or a virtual interface",
    );
}

#[test]
fn interface_array_ports_connect_and_are_values() {
    for (fixture, expected) in [
        (
            "interface_array_ports",
            include_str!("fixtures/sim/virtual_interfaces/interface_array_ports.out"),
        ),
        (
            "interface_array_ports_2d",
            include_str!("fixtures/sim/virtual_interfaces/interface_array_ports_2d.out"),
        ),
    ] {
        sim_cli::run_case_backend_parity("virtual_interfaces", fixture, expected, &[], &[]);
        sim_cli::run_case_after_db_drop("virtual_interfaces", fixture, expected);
    }
    sim_cli::reject_case(
        "virtual_interfaces",
        "neg_interface_array_2d",
        "neg_interface_array_2d.sv:10:9 a multidimensional fixed array in `tb` takes only nested assignment patterns",
    );
}

/// Unpacked members used through a handle name the member of the instance
/// the handle binds where the use is evaluated (SV 25.9).
#[test]
fn unpacked_members_are_values_and_targets() {
    sim_cli::run_case_with_args(
        "virtual_interfaces",
        "member_arrays",
        include_str!("fixtures/sim/virtual_interfaces/member_arrays.out"),
        "",
        &["member_arrays.sv:135:13 repeat count in `tb` is not a constant; evaluated at runtime"],
        // The model takes the C compiler's optimizer, which is not under test,
        // a minute.
        &["--model-opt-level", "O1"],
    );
    sim_cli::run_case(
        "virtual_interfaces",
        "member_array_forms",
        include_str!("fixtures/sim/virtual_interfaces/member_array_forms.out"),
        "INFO: member_array_forms.sv:102:5: [SIM-001] (tb @ 7 ns) $finish called\n",
        &[],
    );
}

/// Implicit sensitivity and explicit waits on unpacked members through a
/// handle follow the instance it binds when the wait arms (SV 9.4.2, 25.9).
#[test]
fn unpacked_member_waits_follow_the_bound_instance() {
    sim_cli::run_case_backend_parity(
        "virtual_interfaces",
        "member_array_sensitivity",
        include_str!("fixtures/sim/virtual_interfaces/member_array_sensitivity.out"),
        &[],
        &[],
    );
    sim_cli::run_case(
        "virtual_interfaces",
        "member_array_waits",
        include_str!("fixtures/sim/virtual_interfaces/member_array_waits.out"),
        "INFO: member_array_waits.sv:51:8: [SIM-001] (tb @ 11 ns) $finish called\n",
        &[],
    );
}

/// A timed blocking write locates its target when its control has passed
/// (SV 10.4.1), through a handle as through a variable; decision VM-D2 has
/// the nonblocking forms, which locate it at issue.
#[test]
fn unpacked_member_timed_writes_locate_their_target_after_the_control() {
    sim_cli::run_case(
        "virtual_interfaces",
        "member_array_timed_blocking",
        include_str!("fixtures/sim/virtual_interfaces/member_array_timed_blocking.out"),
        "INFO: member_array_timed_blocking.sv:49:5: [SIM-001] (tb @ 5 ns) $finish called\n",
        &[],
    );
}

/// A use through a computed handle in a statement whose calls change the
/// handle reads it once, where the use starts in the operand order
/// (decision VM-D1).
#[test]
fn unpacked_member_uses_read_a_computed_handle_in_operand_order() {
    sim_cli::run_case(
        "virtual_interfaces",
        "member_array_order",
        include_str!("fixtures/sim/virtual_interfaces/member_array_order.out"),
        "INFO: member_array_order.sv:129:8: [SIM-001] (tb @ 2 ns) $finish called\n",
        &[],
    );
}

#[test]
fn unpacked_member_of_a_null_handle_fails_at_runtime() {
    sim_cli::run_case_failing_at_runtime(
        "virtual_interfaces",
        "member_array_null",
        &[],
        "1 4\n",
        "member_array_null.sv:17:14 virtual interface access failed: virtual interface member \
         access",
    );
}
