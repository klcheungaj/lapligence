use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_014";

#[test]
fn foreach_mixed_dimensions_bounds_and_captures() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/foreach_scopes.out");
    sim_cli::run_case(SUITE, "foreach_scopes", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "foreach_scopes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "foreach_scopes", expected);
}

#[test]
fn reductions_keep_element_and_map_widths() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/reduction_widths.out");
    sim_cli::run_case(SUITE, "reduction_widths", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "reduction_widths", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "reduction_widths", expected);
}

#[test]
fn membership_known_matches_dominate_and_rows_are_traversed() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/membership.out");
    sim_cli::run_case(SUITE, "membership", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "membership", expected, &[], &[]);
}

#[test]
fn queries_cover_selected_formal_and_descriptor_dimensions() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/queries.out");
    sim_cli::run_case(SUITE, "queries", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "queries", expected, &[], &[]);
}

#[test]
fn cell_wise_ordering_of_rows_records_formals_and_selections() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/cell_ordering.out");
    sim_cli::run_case(SUITE, "cell_ordering", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "cell_ordering", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "cell_ordering", expected);
}

#[test]
fn descriptor_methods_stay_cell_wise() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;

    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_014/descriptor_methods.out");
    sim_cli::run_case(SUITE, "descriptor_methods", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_methods", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "descriptor_methods", expected);

    // Unrolled or flattened traversal would grow with the 65,537-cell
    // extent; cell-wise loops keep the generated model bounded.
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("rtl014-descriptor").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_014/descriptor_methods.sv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .arg(source)
            .args(["--top", "tb", "--gen-only", "--out-dir"]);
        command.arg(directory.path());
        if !optimized {
            command.arg("--no-opt");
        }
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .expect("generate descriptor model");
        assert!(output.status.success(), "{output:?}");
        let model =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(
            model.len() < 200_000,
            "unexpected model size: {}",
            model.len()
        );
    }
}

#[test]
fn sort_ties_witness_checks_only_the_allowed_result_set() {
    // Adapted FND-002 witness for L-F07-13-01 (SV2009 7.12.2): equal-key
    // order is unspecified, so the fixture prints only permutation invariants.
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_014/sort_ties.out");
    sim_cli::run_case(SUITE, "sort_ties", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "sort_ties", expected, &[], &[]);
}

#[test]
fn neg_readonly_iterators_and_receivers() {
    // Adopted FND-002 witnesses for L-F05-11-03 and L-F07-13-02.
    sim_cli::reject_case(
        SUITE,
        "neg_iterator_write",
        "cannot assign to read-only variable 'i'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_reverse_with",
        "cannot use 'with' expression with 'reverse'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_ref_receiver",
        "fixed value is not writable",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_const_ref_descriptor",
        "would modify a const ref formal; its receiver is not writable",
    );
}

#[test]
fn neg_maps_members_and_dimensions() {
    // Adopted FND-002 witness for L-F07-11-01: unpacked records are not
    // singular set members.
    sim_cli::reject_case(
        SUITE,
        "struct_inside",
        "invalid type 'T' for inside expression",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_sort",
        "array method 'sort' can only be called on unpacked arrays of comparable values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_index_dimension",
        "has undefined dimension 2; this lexical iterator defines dimensions 1..1",
    );
    // Resource limit, not a language rule: the row item exceeds packed capacity.
    sim_cli::reject_case(
        SUITE,
        "neg_row_key_capacity",
        "requires a supported fixed element for its `with` expression",
    );
}

#[test]
fn neg_verilog_2001_has_no_array_methods() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_methods_2001.v",
        "is not available in IEEE 2001",
        &["--edition", "2001"],
    );
}
