//! Packed integral constant patterns across the public SV2009 simulator path.
use crate::sim_cli;

#[test]
fn whole_packed_struct_and_union_constants_match_runtime_values() {
    for (value, expected) in [
        ("+v=a5", "struct=1 union=1 typed_struct=1 typed_union=1\n"),
        ("+v=5a", "struct=0 union=0 typed_struct=0 typed_union=0\n"),
    ] {
        sim_cli::run_case_with_cli_and_runtime_args(
            "audit_a1_packed_constant_patterns",
            "whole_values",
            expected,
            "",
            &["--edition", "2009"],
            &[value],
        );
    }
}

#[test]
fn nested_tagged_wide_mixed_and_case_modes_keep_integral_semantics() {
    for (args, expected) in [
        (
            vec!["+v=a5", "+w=123456789abcdef012"],
            "nested=1\ntagged=1\nwide=1\nsigned=1\ncase=1\ncasez=1\ncasex=1\nmixed=1\n",
        ),
        (
            vec!["+v=5a", "+w=123456789abcdef013"],
            "nested=0\ntagged=0\nwide=0\nsigned=0\ncase=0\ncasez=0\ncasex=0\nmixed=1\n",
        ),
        (
            vec!["+v=az", "+w=123456789abcdef012"],
            "nested=0\ntagged=0\nwide=1\nsigned=0\ncase=0\ncasez=1\ncasex=1\nmixed=1\n",
        ),
        (
            vec!["+v=ax", "+w=123456789abcdef012"],
            "nested=0\ntagged=0\nwide=1\nsigned=0\ncase=0\ncasez=0\ncasex=1\nmixed=1\n",
        ),
    ] {
        sim_cli::run_case_with_cli_and_runtime_args(
            "audit_a1_packed_constant_patterns",
            "contexts",
            expected,
            "",
            &["--edition", "2009"],
            &args,
        );
    }
}

#[test]
fn nonintegral_constant_patterns_remain_rejected() {
    sim_cli::reject_case_with_args(
        "audit_a1_packed_constant_patterns",
        "unpacked_subject",
        "unpacked",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "audit_a1_packed_constant_patterns",
        "real_constant",
        "integral",
        &["--edition", "2009"],
    );
}

#[test]
fn packed_pattern_syntax_remains_systemverilog_only() {
    sim_cli::reject_case_with_args(
        "audit_a1_packed_constant_patterns",
        "whole_values",
        "undeclared identifier 'matches'",
        &["--edition", "2001"],
    );
}
