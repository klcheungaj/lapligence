//! ADV-008: state-dependent and edge-sensitive module paths, pulse limits,
//! pulse styles and showcancelled. Oracles are hand-derived from IEEE
//! 1364-2001 14.2-14.6 and IEEE 1800-2009 30.4-30.7, as quoted in the fixture
//! readme. Verilog sources run in both editions; every positive case runs
//! both optimizer modes on every value backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_008";
const BOTH: &[&str] = &["v2001", "sv2009"];

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_008/",
            $name
        ))
    };
}

#[test]
fn conditions_are_evaluated_when_the_transition_is_scheduled() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "conditions.v",
            fixture!("conditions.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn ifnone_paths_apply_when_no_other_path_of_their_terminals_is_active() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "ifnone.v",
            fixture!("ifnone.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn edge_sensitive_paths_start_at_the_last_specified_edge() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "edges.v",
            fixture!("edges.out"),
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_backend_parity(
        SUITE,
        "edge_any.sv",
        fixture!("edge_any.out"),
        &["--edition", "sv2009"],
        &[],
    );
}

#[test]
fn pathpulse_limits_reject_filter_or_pass_each_pulse() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "pathpulse.v",
            fixture!("pathpulse.out"),
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "pathpulse.v", fixture!("pathpulse.out"));
}

#[test]
fn showcancelled_and_pulse_styles_place_the_x() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "showcancelled.v",
            fixture!("showcancelled.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn every_pair_of_scheduled_transitions_is_a_pulse() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "trains.v",
            fixture!("trains.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn pulse_filtering_composes_with_distributed_delays_and_force() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "composition.v",
            fixture!("composition.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn invocation_options_set_percent_limits_and_styles() {
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "options.v",
            fixture!("options.out"),
            &["--edition", edition],
            &[],
        );
    }
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "options.v",
            fixture!("options_r40_e80.out"),
            &[
                "--edition",
                edition,
                "--pulse-reject",
                "40",
                "--pulse-error",
                "80",
            ],
            &[],
        );
    }
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "options.v",
            fixture!("options_r34_e34.out"),
            &[
                "--edition",
                edition,
                "--pulse-reject",
                "34",
                "--pulse-error",
                "34",
            ],
            &[],
        );
    }
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "options.v",
            fixture!("options_ondetect.out"),
            &[
                "--edition",
                edition,
                "--pulse-reject",
                "40",
                "--pulse-error",
                "80",
                "--pulse-style",
                "ondetect",
                "--show-cancelled",
            ],
            &[],
        );
    }
}

/// The `[simulator]` keys give the same controls; the command line wins.
#[test]
fn llg_toml_sets_the_pulse_controls() {
    let config = "schema_version = 1\n\n[simulator]\npulse_reject = 40\npulse_error = 80\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "options.v",
        fixture!("options_r40_e80.out"),
        &["--config", "llg.toml"],
        &[],
        &[("llg.toml", config)],
    );
    let styled = "schema_version = 1\n\n[simulator]\npulse_reject = 40\npulse_error = 80\n\
                  pulse_style = \"onevent\"\nshow_cancelled = false\n";
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "options.v",
        fixture!("options_ondetect.out"),
        &[
            "--config",
            "llg.toml",
            "--pulse-style",
            "ondetect",
            "--show-cancelled",
        ],
        &[],
        &[("llg.toml", styled)],
    );
}

#[test]
fn invalid_pulse_controls_are_rejected() {
    sim_cli::reject_case_with_args(
        SUITE,
        "options.v",
        "llg: the pulse error limit (40%) is below the pulse reject limit (80%)",
        &["--pulse-reject", "80", "--pulse-error", "40"],
    );
}

/// IEEE 1364-2001 has no `edge` identifier (14.2.4); the strict edition
/// profile rejects it with its location.
#[test]
fn the_edge_identifier_is_systemverilog_only() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_edge_v2001.v",
        "neg_edge_v2001.v:8:6 `edge event control` is not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}
