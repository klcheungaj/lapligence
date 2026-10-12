//! SIM-034: synchronous drives, cycle delays and clocking outputs through
//! virtual interfaces. Oracles are derived by hand in the fixture readme from
//! IEEE 1800-2009 §§4.4-4.5, 14.11-14.16 and 25.9.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_034";

/// Compile from an owned `CompileOpts` after the frontend `Db` is dropped and
/// compare stdout and the `$finish` report.
fn after_db_drop(fixture: &str, expected: &str, expected_stderr: &str) {
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
        expected_stderr,
    );
}

#[test]
fn drives_and_cycle_delays_follow_the_region_and_time_oracle() {
    // A02: same-edge sampling versus drives, Re-NBA commits after the
    // Reactive region, skewed, cycle-delayed and off-event drives and waits.
    let expected = include_str!("../fixtures/sim/feature_completion/sim_034/drive_timeline.out");
    sim_cli::run_case_backend_parity(SUITE, "drive_timeline", expected, &[], &[]);
    after_db_drop(
        "drive_timeline",
        expected,
        "llg: $finish at time 53 at tb:77:8\n",
    );
}

#[test]
fn virtual_interface_drives_stay_on_the_instance_named_at_issue() {
    // A01: two instances on irregular clocks; output, skewed, cycle-delayed,
    // selected and inout net drives through rebinding, class and array handles.
    let expected = include_str!("../fixtures/sim/feature_completion/sim_034/vif_drives.out");
    sim_cli::run_case_backend_parity(SUITE, "vif_drives", expected, &[], &[]);
    after_db_drop(
        "vif_drives",
        expected,
        "llg: $finish at time 35000 at tb:82:9\n",
    );
}

#[test]
fn virtual_interface_drives_apply_clockvar_selects() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_034/vif_selects.out");
    sim_cli::run_case_backend_parity(SUITE, "vif_selects", expected, &[], &[]);
}

#[test]
fn inout_clockvar_on_a_wire_resolves_with_its_other_driver() {
    // A03: the clocking block's own 'z-initialized driver.
    let expected = include_str!("../fixtures/sim/feature_completion/sim_034/inout_net.out");
    sim_cli::run_case_backend_parity(SUITE, "inout_net", expected, &[], &[]);
}

// FND-002 witness, source unchanged.

#[test]
fn virtual_clocking_witness() {
    // L-F12-02-03, L-F12-04-05: an output drive through a virtual interface.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_034/virtual_clocking_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "virtual_clocking_witness", expected, &[], &[]);
}

// A03: language-illegal neighbours stay diagnostics.

#[test]
fn neg_intra_delay_drive() {
    sim_cli::reject_case(
        SUITE,
        "neg_intra_delay_drive",
        "clocking signals cannot be driven with a timing control other than a cycle delay",
    );
}

#[test]
fn neg_blocking_drive() {
    sim_cli::reject_case(
        SUITE,
        "neg_blocking_drive",
        "can only be written via a synchronous drive",
    );
}

#[test]
fn neg_cycle_plain_nba() {
    sim_cli::reject_case(
        SUITE,
        "neg_cycle_plain_nba",
        "intra-assignment cycle delays can only be used with clocking signals",
    );
}

#[test]
fn neg_cycle_prefix_no_default() {
    sim_cli::reject_case(
        SUITE,
        "neg_cycle_prefix_no_default",
        "cycle delay cannot be used because no default clocking has been specified",
    );
}

#[test]
fn neg_output_dynamic_skew() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_dynamic_skew",
        "reference to non-constant variable 'n' is not allowed in a constant expression",
    );
}

#[test]
fn neg_vif_input_drive() {
    sim_cli::reject_case(
        SUITE,
        "neg_vif_input_drive",
        "cannot write to input clocking signal 'a'",
    );
}

#[test]
fn neg_vif_compound_drive() {
    sim_cli::reject_case(
        SUITE,
        "neg_vif_compound_drive",
        "can only be written via a synchronous drive",
    );
}

#[test]
fn neg_vif_concat_drive() {
    sim_cli::reject_case(
        SUITE,
        "neg_vif_concat_drive",
        "cannot be part of a concatenation or assignment pattern lvalue",
    );
}
