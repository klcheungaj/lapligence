use super::{reject_case, run_case, run_case_with_stderr};

#[test]
fn continuous_delays_cancel_superseded_propagation_events() {
    run_case("inertial_continuous_pulse", "3 x\n6 x\n7 1\n");
}

#[test]
fn gate_delays_cancel_superseded_propagation_events() {
    run_case("inertial_gate_pulse", "3 x\n6 x\n7 0\n");
}

#[test]
fn unchanged_results_keep_deadlines_and_returning_to_current_value_cancels() {
    run_case(
        "inertial_unchanged",
        "stable 0 0\ncanceled 0 0\npropagated 1 1\n",
    );
}

#[test]
fn vector_drivers_capture_and_cancel_whole_four_state_values() {
    run_case("inertial_vector", "3 x x\n4 x x\n5 1 1\nstates 1 1xz0 1\n");
}

#[test]
fn independent_driver_events_resolve_after_their_own_delays() {
    run_case("inertial_drivers", "2 x\n4 0\n8 0\n10 x\n12 1\n");
}

#[test]
fn driver_updates_run_in_active_region_and_postponed_output_waits_for_settling() {
    run_case(
        "inertial_regions",
        "inactive 0\npostponed 1\nactive 1\nsettled 1 0\nlater 0\n",
    );
}

#[test]
fn constant_driver_updates_outlive_their_evaluation_process() {
    run_case_with_stderr(
        "inertial_constant",
        "final 7 2a\n",
        "llg: simulation ended without $finish (no processes remain) at time 7\n",
    );
}

#[test]
fn finish_discards_pending_updates() {
    run_case("inertial_finish_pending", "pending x\nfinal 3\n");
}

#[test]
fn inertial_deadline_overflow_is_diagnosed() {
    reject_case(
        "inertial_overflow",
        "simulation time overflow while scheduling an inertial update",
    );
}

#[test]
fn fractional_driver_delays_use_the_owning_module_precision() {
    run_case("inertial_fractional", "0.3 x x\n0.4 1 x\n2.6 1 1\n");
}

#[test]
fn enable_gate_delays_preserve_unknowns_and_high_impedance() {
    run_case(
        "inertial_enable",
        "disabled z\ncanceled z\nunknown x\nenabled 0\n",
    );
}

#[test]
fn delayed_driver_updates_preserve_strength_and_release_semantics() {
    run_case(
        "inertial_strength",
        "strong 0\nconflict 0\nreleased 1\ncanceled 1\n",
    );
}

#[test]
fn separate_transition_delays_select_rise_fall_and_turn_off() {
    run_case(
        "inertial_transition_delays",
        "t6 0 00 zz00 0000 zz00\nt9 1 11 zz11 1111 zz11\nt12 x xx zzxx xxxx zzxx\nt19 z xx zzxx zzzz zzzz\n",
    );
    for fixture in [
        "inertial_continuous_two_delays",
        "inertial_continuous_three_delays",
        "inertial_gate_two_delays",
        "inertial_gate_three_delays",
    ] {
        run_case(fixture, "");
    }
}
