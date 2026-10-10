//! SIM-023: legal force/release targets and lifetime semantics.
//! Oracles are derived by hand in the fixture readme from V2001 §9.3.2 and
//! IEEE 1800-2009 §§4.9.2, 6.4, 6.21, 10.6.2, 11.5.1 and 13.3.2.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_023";

/// Run a fixture after the frontend snapshot and owned Db are destroyed, at
/// native O0 and O3 in both optimizer modes; the runtime stderr is the
/// `$finish` report at the fixture's own location.
fn after_db_drop(fixture: &str, expected: &str, finish: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(fixture);
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

// ── A01: nets versus variables, partial and overlapping forces ──────────────

#[test]
fn nets_resolve_on_release_and_variables_keep_or_reestablish() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_023/nets_and_variables.out");
    sim_cli::run_case_backend_parity(SUITE, "nets_and_variables", expected, &[], &[]);
    after_db_drop(
        "nets_and_variables.sv",
        expected,
        "llg: $finish at time 5000 at tb:69:5\n",
    );
}

#[test]
fn overlapping_net_selects_keep_neighbouring_bits() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_023/partial_overlap.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "partial_overlap.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
    sim_cli::run_case_backend_parity(SUITE, "partial_overlap.v", expected, &[], &[]);
}

#[test]
fn packed_member_element_and_indexed_net_selects() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "net_member_selects",
        include_str!("../fixtures/sim/feature_completion/sim_023/net_member_selects.out"),
        &[],
        &[],
    );
}

#[test]
fn variable_concatenations_replace_and_release_per_variable() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_023/variable_concat.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "variable_concat.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
    sim_cli::run_case_backend_parity(SUITE, "variable_concat.v", expected, &[], &[]);
}

#[test]
fn selected_net_force_witness() {
    // FND-002 L-F05-13-01 (V2001 9.3.2; SV 10.6.2).
    sim_cli::run_case_backend_parity(
        SUITE,
        "selected_net_force_witness.v",
        include_str!("../fixtures/sim/feature_completion/sim_023/selected_net_force_witness.out"),
        &["--edition", "v2001"],
        &[],
    );
}

// ── A02: hierarchical and alias targets with live inputs ────────────────────

#[test]
fn hierarchical_alias_and_interface_targets_follow_live_inputs() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_023/hier_live.out");
    sim_cli::run_case_backend_parity(SUITE, "hier_live", expected, &[], &[]);
    after_db_drop(
        "hier_live.sv",
        expected,
        "llg: $finish at time 5000 at tb:83:5\n",
    );
}

#[test]
fn hierarchical_targets_in_verilog_2001() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_023/hier_live_v2001.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "hier_live_v2001.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
    sim_cli::run_case_backend_parity(SUITE, "hier_live_v2001.v", expected, &[], &[]);
}

#[test]
fn hierarchical_force_witness() {
    // FND-002 L-F05-13-02, L-F05-13-03 (V2001 9.3.2; SV 10.6.2).
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_023/hierarchical_force_witness.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "hierarchical_force_witness.v",
        expected,
        &["--edition", "v2001"],
        &[],
    );
    sim_cli::run_case_backend_parity(SUITE, "hierarchical_force_witness.v", expected, &[], &[]);
}

// ── A03: lifetime, teardown and illegal references ──────────────────────────

#[test]
fn forces_outlive_killed_and_disabled_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_023/lifetime.out");
    sim_cli::run_case_backend_parity(SUITE, "lifetime", expected, &[], &[]);
    after_db_drop(
        "lifetime.sv",
        expected,
        "llg: $finish at time 8000 at tb:49:5\n",
    );
}

#[test]
fn neg_automatic_target() {
    // SV 6.21, 13.3.2, reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_target",
        "neg_automatic_target.sv:5:11 cannot refer to automatic variable 'a' from non-procedural context",
    );
}

#[test]
fn neg_force_automatic_witness() {
    // FND-002 L-F05-13-02 (SV 6.21, 13.3.2).
    sim_cli::reject_case(
        SUITE,
        "neg_force_automatic_witness",
        "cannot refer to automatic variable 'x' from non-procedural context",
    );
}

#[test]
fn neg_automatic_rhs() {
    // SV 13.3.2: a force statement cannot reference an automatic variable.
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_rhs",
        "neg_automatic_rhs.sv:6:15 cannot refer to automatic variable 'k' from non-procedural context",
    );
}

#[test]
fn neg_ref_formal() {
    // SV 13.3.2: a ref formal of an automatic task.
    sim_cli::reject_case(
        SUITE,
        "neg_ref_formal",
        "neg_ref_formal.sv:5:11 cannot refer to automatic variable 'x' from non-procedural context",
    );
}

#[test]
fn neg_class_property() {
    // SV 6.21, reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_class_property",
        "neg_class_property.sv:10:11 lvalue of force/release must be a net, a variable",
    );
}

#[test]
fn neg_dynamic_element() {
    // SV 6.21, reported by the frontend.
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_element",
        "neg_dynamic_element.sv:7:11 lvalue of force/release must be a net, a variable",
    );
}

#[test]
fn neg_variable_select() {
    // V2001 9.3.2; SV 10.6.2.
    let diagnostic = "neg_variable_select.v:5:17 lvalue of force/release must be a net, a variable";
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_variable_select.v",
        diagnostic,
        &["--edition", "v2001"],
    );
    sim_cli::reject_case(SUITE, "neg_variable_select.v", diagnostic);
}

#[test]
fn neg_force_variable_select_witness() {
    // FND-002 L-F05-13-01 (SV 10.6.2).
    sim_cli::reject_case(
        SUITE,
        "neg_force_variable_select_witness",
        "neg_force_variable_select_witness.sv:6:23 lvalue of force/release must be a net, a variable",
    );
}

#[test]
fn neg_force_dynamic_net_select_witness() {
    // FND-002 L-F05-13-01 (SV 10.6.2: constant net selects only).
    sim_cli::reject_case(
        SUITE,
        "neg_force_dynamic_net_select_witness",
        "reference to non-constant variable 'i' is not allowed in a constant expression",
    );
}

#[test]
fn neg_array_element_in_verilog_2001() {
    // V2001 9.3.2: "It cannot be a memory word (array reference)".
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_array_element.v",
        "neg_array_element.v:4:17 lvalue of force/release must be a net, a variable",
        &["--edition", "v2001"],
    );
}

#[test]
fn static_memory_element_force_is_a_known_limitation() {
    // FND-002 L-F05-13-02 reads an element of a static unpacked array as a
    // singular variable (SV 6.4, 10.6.2). The frontend rejects every select
    // of a variable; see docs/known_issues.md.
    sim_cli::reject_case(
        SUITE,
        "static_memory_force_witness",
        "static_memory_force_witness.sv:6:15 lvalue of force/release must be a net, a variable",
    );
}

#[test]
fn neg_struct_member_variable() {
    // SV 10.6.2: a member of a packed structure variable is a part-select of
    // a variable.
    sim_cli::reject_case(
        SUITE,
        "neg_struct_member_variable",
        "neg_struct_member_variable.sv:9:17 lvalue of force/release must be a net, a variable",
    );
}

#[test]
fn neg_unpacked_array() {
    // SV 10.6.2, 6.4: not a singular variable.
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_array",
        "neg_unpacked_array.sv:4:11` is an unpacked array, which is not a singular variable (IEEE 1800-2009 10.6.2)",
    );
}

#[test]
fn neg_unpacked_struct() {
    // SV 10.6.2, 6.4: not a singular variable.
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_struct",
        "neg_unpacked_struct.sv:8:11` is an unpacked structure or union, which is not a singular variable (IEEE 1800-2009 10.6.2)",
    );
}

#[test]
fn unsupported_string_target() {
    // Legal (6.4: strings are singular) but not supported; docs/known_issues.md.
    sim_cli::reject_case(
        SUITE,
        "neg_string",
        "neg_string.sv:4:11` is not supported; force/release supports packed, real and enumerated variables and nets",
    );
}

#[test]
fn unsupported_class_handle_target() {
    // Legal (6.4: handles are singular) but not supported; docs/known_issues.md.
    sim_cli::reject_case(
        SUITE,
        "neg_class_handle",
        "neg_class_handle.sv:6:11` is not supported",
    );
}

#[test]
fn unsupported_whole_net_array_target() {
    // Not supported; its elements are (`hier_live`, `partial_overlap`).
    sim_cli::reject_case(
        SUITE,
        "neg_whole_net_array",
        "neg_whole_net_array.sv:7:11` is not supported; force its elements instead",
    );
}
