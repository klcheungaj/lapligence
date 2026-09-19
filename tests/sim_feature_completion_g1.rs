//! Group 1 feature-completion slice: nested fixed-array element selections,
//! expression sequencing, and fixed bit-stream forms. Fixtures run through the
//! public simulator in both optimizer modes with independent stdout oracles.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn nested_array_member_select() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_member_select",
        concat!(
            "whole 1122aa44\n",
            "part 11b2aa44\n",
            "bit 10b2aa44\n",
            "indexed 10b2aa4c\n",
            "other deadbeef 10b2aa4c 0c00\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_member_read() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_member_read",
        concat!(
            "lane 44 33 22 11\n",
            "part 2 2\n",
            "bit 1 1\n",
            "indexed 4 4 1\n",
            "asc 11 22 33 44\n",
            "other deadbeef c\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_member_bounds() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_member_bounds",
        concat!(
            "low xx x\n",
            "high xx x\n",
            "unknown xx xxxx\n",
            "read dd b 0\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_member_indexed_plus() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_member_indexed_plus",
        concat!(
            "read_plus 0\n",
            "write_plus 11223354\n",
            "write_plus2 25223354\n",
        ),
        "",
        &[],
    );
}

#[test]
fn selection_unknown_bounds() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "selection_unknown_bounds",
        concat!(
            "bounds 11223344 aabbccdd 3c\n",
            "unknown 11223344 aabbccdd 3c\n",
            "read c aabbccdd\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_runtime_lane() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_runtime_lane",
        concat!(
            "desc dd\n",
            "desc cc\n",
            "desc bb\n",
            "desc aa\n",
            "asc 11\n",
            "asc 22\n",
            "asc 33\n",
            "asc 44\n",
            "bit 0\n",
            "bit 1\n",
            "bit 1\n",
            "wrdesc aa5accdd\n",
            "wrasc 117e3344\n",
            "wrbit 16\n",
            "other 11223344\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_runtime_lane_bounds() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_runtime_lane_bounds",
        concat!(
            "high xx x\n",
            "highwr aabbccdd 5a\n",
            "low xx x\n",
            "lowwr aabbccdd 5a\n",
            "unknown xx x\n",
            "unknownwr aabbccdd 5a\n",
            "elemhigh xx x\n",
            "elemhighwr aabbccdd 5a\n",
            "in bb 0\n",
        ),
        "",
        &[],
    );
}

#[test]
fn nested_array_runtime_lane_chain() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "nested_array_runtime_lane_chain",
        "sel cc\n",
        "",
        &[],
    );
}

#[test]
fn overlap_concat_store() {
    sim_cli::run_case(
        "feature_completion/g1_12",
        "overlap_concat_store",
        concat!(
            "concat f0\n",
            "concat_overlap 69\n",
            "stream 22 11\n",
            "part a5a5\n",
        ),
        "",
        &[],
    );
}

#[test]
fn stream_slice_tail() {
    sim_cli::run_case(
        "feature_completion/g1_26",
        "stream_slice_tail",
        concat!("rhs 1d983a\n", "lhs a1 b2 c3\n"),
        "",
        &[],
    );
}

#[test]
fn stream_dynamic_fixed_selection() {
    sim_cli::run_case(
        "feature_completion/g1_26",
        "stream_dynamic_fixed_selection",
        concat!(
            "plus 00 a4 b5\n",
            "minus 00 d2 c1\n",
            "index ee 00 00\n",
            "range 00 12 34\n",
            "desc 00 55 66\n",
            "reverse 00 88 77\n",
            "overlap 11 11 22\n",
        ),
        "",
        &[],
    );
}

#[test]
fn stream_dynamic_fixed_selection_bounds() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "feature_completion/g1_26",
            "stream_dynamic_fixed_selection_bounds",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "optimized={optimized}: {stderr}"
        );
        assert_eq!(
            stderr
                .matches(
                    "fixed streaming target selector is unknown, empty, or outside declared bounds"
                )
                .count(),
            2,
            "{stderr}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            concat!(
                "oob 11 aa calls=0\n",
                "unknown 11 22\n",
                "once 00 ee calls=1\n",
            ),
            "optimized={optimized}: {stderr}"
        );
    }
}

#[test]
fn stream_runtime_fixed_source() {
    sim_cli::run_case(
        "feature_completion/g1_26",
        "stream_runtime_fixed_source",
        concat!(
            "plus 2233\n",
            "minus 3322\n",
            "index 0011\n",
            "range 2233\n",
            "desc aabb\n",
            "rev 2211\n",
            "rev4 2211\n",
        ),
        "",
        &[],
    );
}

#[test]
fn stream_runtime_fixed_source_bounds() {
    sim_cli::run_case(
        "feature_completion/g1_26",
        "stream_runtime_fixed_source_bounds",
        concat!(
            "oob xxxx\n",
            "partial 33xx\n",
            "below xxxx\n",
            "unknown 0000\n",
            "once 5aa5 calls=1\n",
            "overlap 11 11 22\n",
        ),
        "",
        &[],
    );
}

#[test]
fn stream_runtime_fixed_source_multidim_rejected() {
    sim_cli::reject_case(
        "feature_completion/g1_26",
        "stream_runtime_fixed_source_multidim",
        "runtime `with` selector on a multidimensional fixed streaming source is not supported",
    );
}

#[test]
fn bitstream_size_mismatch() {
    sim_cli::reject_case(
        "feature_completion/g1_26",
        "bitstream_size_mismatch",
        "cannot be converted",
    );
}

#[test]
fn compound_index_once() {
    sim_cli::run_case(
        "feature_completion/g1_31",
        "compound_index_once",
        "compound mem=13 i=3 calls=1 old=13\n",
        "",
        &[],
    );
}

#[test]
fn statement_select_mutation() {
    sim_cli::run_case(
        "feature_completion/g1_31",
        "statement_select_mutation",
        concat!(
            "compound mem2=13 i=3 calls=1\n",
            "postinc mem1=3 i=1\n",
            "preinc mem3=5 i=3\n",
            "part word=0020\n",
            "part2 word=0f20\n",
            "dec mem0=0 i=0\n",
        ),
        "",
        &[],
    );
}

#[test]
fn statement_select_mutation_bounds() {
    sim_cli::run_case(
        "feature_completion/g1_31",
        "statement_select_mutation_bounds",
        concat!(
            "oob 10 20 calls=1\n",
            "unknown 10 20 calls=2\n",
            "in 10 21 calls=3\n",
        ),
        "",
        &[],
    );
}

#[test]
fn short_circuit_effects() {
    sim_cli::run_case(
        "feature_completion/g1_31",
        "short_circuit_effects",
        concat!(
            "and 0 1\n",
            "or 1 1\n",
            "and_effect 0 1 0\n",
            "or_effect 1 1 1\n",
        ),
        "",
        &[],
    );
}

#[test]
fn postincrement_result() {
    sim_cli::run_case(
        "feature_completion/g1_31",
        "postincrement_result",
        concat!(
            "post_inc old=5 a=6 after=6\n",
            "pre_inc old=6 a=6 after=6\n",
            "post_dec old=5 a=4 after=4\n",
            "pre_dec old=4 a=4 after=4\n",
        ),
        "",
        &[],
    );
}

// ── G1-22 module ports and hierarchy connectivity ─────────────────────────

#[test]
fn port_label_actual_scope() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_label_actual_scope",
        "p=1 c=1 r=1\np=0 c=0 r=0\n",
        "",
        &[],
    );
}

#[test]
fn port_array_aggregate_roundtrip() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_array_aggregate_roundtrip",
        "y0=-6 y1=-101\ny0=300 y1=27\n",
        "",
        &[],
    );
}

#[test]
fn port_array_element_actual_roundtrip() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_array_element_actual",
        "d=11 22\nd=31 22\n",
        "",
        &[],
    );
}

#[test]
fn port_array_nonconstant_actual_is_rejected() {
    sim_cli::reject_case(
        "feature_completion/g1_22",
        "port_array_nonconstant_actual",
        "non-constant element actual",
    );
}

#[test]
fn port_inout_not_two_copies() {
    sim_cli::run_case(
        "feature_completion/g1_22",
        "port_inout_not_two_copies",
        "none=z\nparent=1\nchild=0\nconflict=x\nrelease=z\n",
        "",
        &[],
    );
}

// ── G1-23 interface and modport usage ─────────────────────────────────────

#[test]
fn interface_shared_modports() {
    sim_cli::run_case(
        "feature_completion/g1_23",
        "interface_shared_modports",
        "d=2a dd=2b bus=2a\nd=40 dd=41 bus=40\n",
        "",
        &[],
    );
}

#[test]
fn interface_array_parameter() {
    sim_cli::run_case(
        "feature_completion/g1_23",
        "interface_array_parameter",
        "got=a0 a1\n",
        "",
        &[],
    );
}

#[test]
fn modport_access_error() {
    sim_cli::reject_case(
        "feature_completion/g1_23",
        "modport_access_error",
        "cannot assign to input port",
    );
}

#[test]
fn modport_access_error_nested() {
    sim_cli::reject_case(
        "feature_completion/g1_23",
        "modport_access_error_nested",
        "cannot assign to input port",
    );
}
