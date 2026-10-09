//! SIM-013: evaluated-event and implicit dependency environments. Oracles
//! are derived by hand in the fixture readme from IEEE 1800-2009 §§6.14,
//! 9.2.2.2, 9.4.2, 9.4.3, 13.4, 13.5.2, 25.9 and 35.5.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_013";

#[test]
fn nested_members_elements_and_sizes_wake_only_their_readers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/nested_values.out");
    sim_cli::run_case(SUITE, "nested_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "nested_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "nested_values", expected);
}

#[test]
fn class_property_waits_follow_the_object_their_handle_names() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/class_handles.out");
    sim_cli::run_case(SUITE, "class_handles", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "class_handles", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "class_handles", expected);
}

#[test]
fn interface_member_waits_follow_the_bound_instance() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/interfaces.out");
    sim_cli::run_case(SUITE, "interfaces", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "interfaces", expected, &[], &[]);
}

#[test]
fn descriptor_array_selectors_move_the_observed_cell() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_013/descriptor_selectors.out");
    sim_cli::run_case(SUITE, "descriptor_selectors", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_selectors", expected, &[], &[]);
}

#[test]
fn handle_formals_and_class_held_interfaces_compose() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "composition", expected);
}

#[test]
fn always_comb_and_at_star_keep_distinct_sensitivity() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/comb_sensitivity.out");
    sim_cli::run_case(SUITE, "comb_sensitivity", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "comb_sensitivity", expected, &[], &[]);
}

#[test]
fn edges_with_unknown_values_and_trigger_time_qualifiers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/edges.out");
    sim_cli::run_case(SUITE, "edges", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "edges", expected, &[], &[]);
}

#[test]
fn helper_events_wait_in_their_own_activation() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_013/helper_activations.out");
    sim_cli::run_case(SUITE, "helper_activations", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "helper_activations", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "helper_activations", expected);
}

/// Build `foreign_helpers.c` into a shared library in `directory`.
fn foreign_library(directory: &sim_harness::TempDir) -> String {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/sim_013/foreign_helpers.c");
    let library = directory.path().join("libsim013_foreign.so");
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let output = sim_harness::run_command(
        std::process::Command::new(compiler)
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&library),
        std::time::Duration::from_secs(60),
    )
    .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    library.to_string_lossy().into_owned()
}

#[test]
fn foreign_helpers_run_in_the_waiting_process() {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = sim_harness::TempDir::new("sim013-dpi").expect("temporary directory");
    let library = foreign_library(&directory);
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/foreign_helpers.out");
    sim_cli::run_case_with_args(
        SUITE,
        "foreign_helpers",
        expected,
        "",
        &[],
        &["--dpi-lib", &library],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "foreign_helpers",
        expected,
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn context_import_in_strobe_without_exports() {
    // SV 4.4.2.9 forbids writes in Postponed; without a DPI export the
    // context import has no SystemVerilog subroutine to write through.
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = sim_harness::TempDir::new("sim013-ctx").expect("temporary directory");
    let library = foreign_library(&directory);
    let expected = include_str!("../fixtures/sim/feature_completion/sim_013/context_strobe.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "context_strobe",
        expected,
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn class_handle_sensitivity_witness() {
    // Adopted FND-002 witness for L-F06-02-01 and L-F02-13-05 (SV2009 9.4.2).
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_013/class_sensitivity_witness.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_sensitivity_witness",
        expected,
        &["--edition", "sv2009"],
        &[],
    );
}

#[test]
fn automatic_event_capture_witness() {
    // Adopted FND-002 witness for L-F06-03-02 (SV2009 9.4.2, 6.21).
    let expected = include_str!(
        "../fixtures/sim/feature_completion/sim_013/automatic_event_capture_witness.out"
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "automatic_event_capture_witness",
        expected,
        &["--edition", "sv2009"],
        &[],
    );
}

#[test]
fn real_anychange_witness() {
    // Adopted FND-002 witness for L-F06-03-01 (V2001 3.9.1, 9.7.2): a real
    // any-change event is legal.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_013/real_anychange_witness.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_anychange_witness.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn real_boolean_edge_witness() {
    // Adopted FND-002 witness for L-F06-03-01 (V2001 3.9.1, 4.1.8, 9.7.2):
    // the edge of a Boolean derived from a real is legal.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_013/real_boolean_edge_witness.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_boolean_edge_witness.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
}

#[test]
fn neg_chandle_event() {
    // Language rule (SV 6.14); also the FND-002 neg_chandle_event witness.
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_event",
        "a chandle cannot be used in an event expression (IEEE 1800-2009 6.14)",
    );
}

#[test]
fn neg_real_edge() {
    // Language rule (SV 6.12.1, 9.4.2), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_real_edge",
        "expression type 'real' is not integral",
    );
}

#[test]
fn neg_output_helper() {
    // Language rule (SV 13.4), reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_output_helper",
        "cannot call a function with output, inout, or ref arguments from an event expression",
    );
}

#[test]
fn neg_context_strobe() {
    // Implementation restriction (SV 4.4.2.9 with 35.5.3): the design has a
    // DPI export the context import may write through, and llg cannot see
    // whether the foreign code does so in the read-only Postponed region.
    sim_cli::reject_case(
        SUITE,
        "neg_context_strobe",
        "may write SystemVerilog storage through exported subroutines",
    );
}
