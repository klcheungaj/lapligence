//! SIM-014: repeated waits, intra-assignment timing and event triggers.
//! Oracles are derived by hand in the fixture readme from V2001 §§9.6,
//! 9.7.7 and IEEE 1800-2009 §§4.9.3, 4.9.4, 6.12.2, 6.21, 9.4.5, 10.4,
//! 12.7.2, 13.3.2, 13.4 and 15.5.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_014";

/// Run a fixture after the frontend snapshot and owned Db are destroyed; the
/// runtime stderr is the `$finish` report at the fixture's own location.
fn after_db_drop(fixture: &str, expected: &str, finish: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        fixture,
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        finish,
    );
}

#[test]
fn standalone_repeat_counts_follow_one_conversion() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_counts",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_counts.out"),
        &[],
        &[],
    );
}

#[test]
fn intra_assignment_repeat_counts_and_issue_time_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "intra_counts",
        include_str!("../fixtures/sim/feature_completion/sim_014/intra_counts.out"),
        &[],
        &[],
    );
}

#[test]
fn blocking_destinations_resolve_at_update_and_nba_destinations_at_issue() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/destinations.out");
    sim_cli::run_case_backend_parity(SUITE, "destinations", expected, &[], &[]);
    after_db_drop(
        "destinations",
        expected,
        "INFO: destinations.sv:116:5: [SIM-001] (tb @ 81 ns) $finish called\n",
    );
}

#[test]
fn blocking_container_element_destinations_resolve_at_update() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "container_elements",
        include_str!("../fixtures/sim/feature_completion/sim_014/container_elements.out"),
        &[],
        &[],
    );
}

#[test]
fn blocking_selects_inside_container_elements_resolve_at_update() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "container_element_selects",
        include_str!("../fixtures/sim/feature_completion/sim_014/container_element_selects.out"),
        &[],
        &[],
    );
}

#[test]
fn nonblocking_triggers_keep_the_event_named_at_issue() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/nb_triggers.out");
    sim_cli::run_case_backend_parity(SUITE, "nb_triggers", expected, &[], &[]);
    after_db_drop(
        "nb_triggers",
        expected,
        "INFO: nb_triggers.sv:60:8: [SIM-001] (tb @ 37 ns) $finish called\n",
    );
}

#[test]
fn mixed_lists_and_event_formals_of_tasks_functions_and_methods() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_formals",
        include_str!("../fixtures/sim/feature_completion/sim_014/event_formals.out"),
        &[],
        &[],
    );
}

#[test]
fn output_and_inout_event_formals_bind_class_event_properties() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_event_formals",
        include_str!("../fixtures/sim/feature_completion/sim_014/class_event_formals.out"),
        &[],
        &[],
    );
}

#[test]
fn ref_event_formals_bind_class_event_properties() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_event_ref_formal",
        include_str!("../fixtures/sim/feature_completion/sim_014/class_event_ref_formal.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "string_element_events",
        include_str!("../fixtures/sim/feature_completion/sim_014/string_element_events.out"),
        &[],
        &[],
    );
    // A collection at every safe point must keep a dropped object alive
    // while a callee holds its property.
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_event_ref_held",
        include_str!("../fixtures/sim/feature_completion/sim_014/class_event_ref_held.out"),
        &[],
        &[("LLG_GC_STRESS", "1")],
    );
}

#[test]
fn event_formals_of_value_calls() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_formals_value_calls",
        include_str!("../fixtures/sim/feature_completion/sim_014/event_formals_value_calls.out"),
        &[],
        &[],
    );
}

#[test]
fn triggered_state_identity_and_wait_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "event_values",
        include_str!("../fixtures/sim/feature_completion/sim_014/event_values.out"),
        &[],
        &[],
    );
}

#[test]
fn failed_wait_order_without_else_ends_the_run() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_014/wait_order_error.out");
    sim_cli::run_case_checked_matrix(SUITE, "wait_order_error", &[], &|label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert!(
            crate::sim_harness::has_message(
                &stderr,
                "SIM-009",
                ":9:9",
                "wait_order: events triggered out of order"
            ),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn cancelled_repeated_waits_leave_no_pending_work() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cancellation",
        include_str!("../fixtures/sim/feature_completion/sim_014/cancellation.out"),
        &[],
        &[],
    );
}

#[test]
fn intra_assignment_event_controls_in_task_bodies() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "task_timing",
        include_str!("../fixtures/sim/feature_completion/sim_014/task_timing.out"),
        &[],
        &[],
    );
}

#[test]
fn verilog_2001_repeated_event_controls() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "v2001_forms.v",
        include_str!("../fixtures/sim/feature_completion/sim_014/v2001_forms.out"),
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn repeat_real_witness() {
    // FND-002 L-F06-05-03: an integral-valued real count.
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_real_witness.v",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_real_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn persistent_repeat_nba_witness() {
    // FND-002 L-F06-04-04: a repeated-event string NBA.
    sim_cli::run_case_backend_parity(
        SUITE,
        "persistent_repeat_nba_witness",
        include_str!(
            "../fixtures/sim/feature_completion/sim_014/persistent_repeat_nba_witness.out"
        ),
        &[],
        &[],
    );
}

#[test]
fn detached_repeat_nba_commits_after_the_last_event() {
    // FIN-001 D1: string, real, class-handle, process-evaluated packed and
    // `->>` forms observed between the events of `repeat (n) @(...)`.
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_nba_progress",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_nba_progress.out"),
        &[],
        &[],
    );
}

#[test]
fn repeat_nested_timing_witness() {
    // FND-002 L-F06-04-01 / L-F06-05-02: a repeat loop around a repeated
    // intra-assignment control.
    sim_cli::run_case_backend_parity(
        SUITE,
        "repeat_nested_timing_witness",
        include_str!("../fixtures/sim/feature_completion/sim_014/repeat_nested_timing_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn trigger_repeat_witness() {
    // FND-002: `->> repeat (2) @(posedge clk) e`.
    sim_cli::run_case_backend_parity(
        SUITE,
        "trigger_repeat_witness",
        include_str!("../fixtures/sim/feature_completion/sim_014/trigger_repeat_witness.out"),
        &[],
        &[],
    );
}

#[test]
fn neg_class_property_nba() {
    // Language rule (SV 6.21), reported by llg with its location.
    sim_cli::reject_case(
        SUITE,
        "neg_class_property_nba",
        "neg_class_property_nba.sv:10:14 nonblocking assignment to class property `x` in `tb` is illegal (IEEE 1800-2009 6.21",
    );
}

#[test]
fn neg_class_record_property_nba() {
    // Language rule (SV 6.21) for a record property with a string member,
    // whole or one member, in both record storage forms (FIN-001 R2).
    for cells in ["N=3", "N=16"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_class_record_property_nba_whole",
            "neg_class_record_property_nba_whole.sv:16:8 nonblocking assignment to class property `r` in `tb` is illegal (IEEE 1800-2009 6.21",
            &["-D", cells],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_class_record_property_nba_member",
            "neg_class_record_property_nba_member.sv:16:8 nonblocking assignment to class property `r` in `tb` is illegal (IEEE 1800-2009 6.21",
            &["-D", cells],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_class_record_property_nba_string",
            "neg_class_record_property_nba_string.sv:16:8 nonblocking assignment to class property `r` in `tb` is illegal (IEEE 1800-2009 6.21",
            &["-D", cells],
        );
    }
}

/// FIN-001 R2: a `ref int` formal a task waits on, bound to a member of a
/// record property or to an element of a container property, resumes on a
/// direct store, in both record storage forms.
#[test]
fn waited_ref_formals_bound_into_class_properties_resume() {
    let members = include_str!(
        "../fixtures/sim/feature_completion/sim_014/ref_wait_record_property_member.out"
    );
    let elements = include_str!(
        "../fixtures/sim/feature_completion/sim_014/ref_wait_container_property_element.out"
    );
    for cells in ["N=3", "N=16"] {
        sim_cli::run_case_backend_parity(
            SUITE,
            "ref_wait_record_property_member",
            members,
            &["-D", cells],
            &[],
        );
        sim_cli::run_case_backend_parity(
            SUITE,
            "ref_wait_container_property_element",
            elements,
            &["-D", cells],
            &[],
        );
    }
}

/// FIN-001 R2: waited `ref` formals bound into a record property that no
/// store would resume reject at the call instead of waiting forever.
#[test]
fn neg_ref_wait_record_property() {
    for cells in ["N=3", "N=16"] {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_ref_wait_record_property_whole",
            "neg_ref_wait_record_property_whole.sv:20:",
            &["-D", cells],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_ref_wait_record_property_real",
            "neg_ref_wait_record_property_real.sv:21:9 storage of record class property `r` bound to a `ref` formal its task waits on cannot be observed by a wait or event expression",
            &["-D", cells],
        );
    }
}

#[test]
fn neg_automatic_event_nba() {
    // Language rule (SV 6.21, 10.4.2), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_event_nba",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
}

#[test]
fn neg_automatic_nba_witness() {
    // FND-002 L-F06-04-02: a queued automatic real write.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_nba_witness",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
}

#[test]
fn neg_automatic_in_nba_control() {
    // Language rule (SV 13.3.2), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_in_nba_control",
        "cannot refer to automatic variable 'w' from intra-assignment event control of nonblocking assignment",
    );
}

#[test]
fn neg_dynamic_element_nba() {
    // Language rule (SV 6.21), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_element_nba",
        "nonblocking assignments to elements of dynamically sized arrays are not allowed",
    );
}

#[test]
fn neg_function_event_control() {
    // Language rule (SV 13.4), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_function_event_control",
        "statements that pass time are not allowed in this context",
    );
}

#[test]
fn event_variable_comparisons_in_waits_reject() {
    let reason = "an event variable comparison cannot be observed by a wait";
    sim_cli::reject_case(SUITE, "reject_event_wait_compare", reason);
    sim_cli::reject_case(SUITE, "reject_event_ref_wait_compare", reason);
    sim_cli::reject_case(SUITE, "reject_event_control_compare", reason);
}

#[test]
fn storage_with_no_change_marker_rejects_in_waits() {
    let reason = "cannot be observed by a wait or event expression";
    sim_cli::reject_case(SUITE, "reject_wait_call_result", reason);
    // An automatic record a fork shares has no marker (FIN-001 R0).
    let reason = "automatic record `r` shared with a fork cannot be observed by a wait";
    sim_cli::reject_case(SUITE, "reject_wait_automatic_record", reason);
    sim_cli::reject_case(SUITE, "reject_event_automatic_record", reason);
    // Nothing stores into the record while a branch waits: it is read as it is.
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_014/wait_automatic_record_unchanged.out"
    );
    sim_cli::run_case(SUITE, "wait_automatic_record_unchanged", expected, "", &[]);
}

/// FIN-001 R0: a member of an unpacked structure property with a string
/// member is waited on through the record's contents marker.
#[test]
fn waits_on_record_property_members_resume() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/wait_record_property.out");
    sim_cli::run_case(SUITE, "wait_record_property", expected, "", &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/event_record_property.out");
    sim_cli::run_case(SUITE, "event_record_property", expected, "", &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/method_wait_record_property.out");
    sim_cli::run_case(SUITE, "method_wait_record_property", expected, "", &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/callee_wait_record_property.out");
    sim_cli::run_case(SUITE, "callee_wait_record_property", expected, "", &[]);
}

#[test]
fn every_store_into_a_record_property_reaches_its_waiters() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/record_property_stores.out");
    sim_cli::run_case(SUITE, "record_property_stores", expected, "", &[]);
}

#[test]
fn waits_on_static_subroutine_record_members_resume() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/static_record_waits.out");
    sim_cli::run_case(SUITE, "static_record_waits", expected, "", &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/static_record_nba_waits.out");
    sim_cli::run_case(SUITE, "static_record_nba_waits", expected, "", &[]);
}

#[test]
fn waits_on_records_in_containers_resume() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_014/wait_container_records.out");
    sim_cli::run_case(SUITE, "wait_container_records", expected, "", &[]);
}

/// Only a model that observes a record calls the record marker entry points:
/// the same design registers a record watch for a `ref` actual when a wait
/// reads the record, and emits none of the calls when nothing does, so a
/// design without such a wait keeps the code it had.
#[test]
fn only_a_model_that_observes_a_record_emits_record_marker_calls() {
    let source = |observer: &str| {
        format!(
            "module tb;\n\
             timeunit 1ns; timeprecision 1ns;\n\
             typedef struct {{ int a; string s; }} rec_t;\n\
             task automatic rf(ref rec_t o); #1 o.s = \"rf\"; endtask\n\
             task keep(input int v);\n\
             static rec_t r;\n\
             r.a = v;\n\
             endtask\n\
             {observer}\n\
             initial begin\n\
             #1 keep(1);\n\
             #1 rf(keep.r);\n\
             #1 $display(\"%0t s=%s\", $time, keep.r.s);\n\
             $finish(0);\n\
             end\n\
             endmodule\n"
        )
    };
    let observed = sim_harness::run_generated_sim(
        &source("initial begin wait (keep.r.s == \"rf\"); $display(\"%0t woke\", $time); end"),
        "tb",
        "sim014_record_marker_observed",
    )
    .expect("observed record");
    assert_eq!(observed.stdout, "3 woke\n4 s=rf\n");
    for call in ["llg_record_watch_begin(", "llg_record_changed("] {
        assert!(observed.model_c.contains(call), "missing {call}");
    }
    let unobserved =
        sim_harness::run_generated_sim(&source(""), "tb", "sim014_record_marker_unobserved")
            .expect("unobserved record");
    assert_eq!(unobserved.stdout, "4 s=rf\n");
    assert!(unobserved.model_c.contains("llg_native_ref_value_stored("));
    for call in [
        "llg_record_watch",
        "llg_record_changed",
        "llg_nba_locate_marker",
    ] {
        assert!(!unobserved.model_c.contains(call), "unexpected {call}");
    }
}
