//! ADV-007: specify blocks, specparams and simple module paths. Oracles are
//! hand-derived from IEEE 1364-2001 14.2-14.4 (Tables 47 and 48) and IEEE
//! 1800-2009 30.4-30.6 (Tables 30-2 and 30-3), as quoted in the fixture
//! readme. Verilog sources run in both editions; every positive case runs
//! both optimizer modes on every value backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_007";
const BOTH: &[&str] = &["v2001", "sv2009"];

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_007/",
            $name
        ))
    };
}

#[test]
fn every_delay_form_selects_the_delay_of_each_transition() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "transitions.v",
            fixture!("transitions.out"),
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "transitions.v", fixture!("transitions.out"));
}

#[test]
fn the_latest_input_selects_the_path_and_vectors_connect_per_bit() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "connections.v",
            fixture!("connections.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn duplicate_and_overlapping_paths_take_the_smallest_delay() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "duplicate_paths.v",
            fixture!("duplicate_paths.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn the_larger_of_path_and_distributed_delays_is_used() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "distributed.v",
            fixture!("distributed.out"),
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "distributed.v", fixture!("distributed.out"));
}

#[test]
fn specparams_give_path_delays_in_both_scopes() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "specparams.v",
            fixture!("specparams.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn path_destinations_filter_pulses_shorter_than_the_delay() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "pulses.v",
            fixture!("pulses.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn forced_destinations_restart_from_the_released_value() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "force_release.v",
            fixture!("force_release.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn in_module_drivers_resolve_before_the_path_in_systemverilog() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "wired.v",
        fixture!("wired.out"),
        &["--edition", "sv2009"],
        &[],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "wired.v",
        "has more than one driver inside the module (IEEE 1364-2001 14.2.1",
        &["--edition", "v2001"],
    );
}

/// Destinations the path mechanism cannot delay exactly are rejected.
#[test]
fn unsupported_destinations_are_rejected_with_their_location() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_variable_destination.v",
            "a variable destination is not simulated yet",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_outside_driver.v",
            "the destination is also driven outside the module",
            &["--edition", edition],
        );
    }
    sim_cli::reject_case(
        SUITE,
        "neg_multidimensional_select.sv",
        "the terminal is not a whole port or a constant bit or part select of one",
    );
}

/// Specparam scoping and terminal errors carry the frontend's source
/// location; llg makes the parameter-in-specify warning an error (A7-D5).
#[test]
fn specify_errors_are_reported_at_their_source() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_parameter_in_specify.v",
            "neg_parameter_in_specify.v:6:16 parameters cannot be used in specify blocks",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_specparam_in_parameter.v",
            "neg_specparam_in_parameter.v:4:17 specparams cannot be used in general constant",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_specparam_in_range.v",
            "neg_specparam_in_range.v:4:9 specparams cannot be used in general constant",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_specparam_before_declaration.v",
            "neg_specparam_before_declaration.v:4:10 identifier 'S' used before its declaration",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_source_not_port.v",
            "neg_source_not_port.v:6:6 specify path source 'w' must be an 'input' or 'inout'",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_destination_input.v",
            "neg_destination_input.v:5:11 specify path destination 'b' must be an 'output'",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_parallel_width.v",
            "neg_parallel_width.v:5:8 parallel specify path source type 'logic[1:0]' has different width",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_delay_count.v",
            "neg_delay_count.v:5:26 expected one, two, three, six, or twelve delay values",
            &["--edition", edition],
        );
    }
}
