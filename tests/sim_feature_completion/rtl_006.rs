//! RTL-006: wide arithmetic at actual widths and single-evaluation mutations.
//!
//! Arithmetic expectations come from the independent limb oracle in
//! `rtl_006/oracle.rs`, never from simulator output; the mutation fixture's
//! `.out` file is hand-derived.

use super::sim_cli;
use std::fmt::Write;

#[path = "rtl_006/oracle.rs"]
mod oracle;

use oracle::{arith, bin, count_ones, hex, negate, operand, power, shift, slice, Op, V};

const SUITE: &str = "feature_completion/rtl_006";
const OPERANDS: usize = 13;
/// Operands 11 and 12 carry X/Z bits; shifting them leaves known bits beside
/// unknown ones, which the fixtures print in binary.
const FIRST_UNKNOWN: usize = 11;

fn hex_list(values: &[V]) -> String {
    values.iter().map(hex).collect::<Vec<_>>().join(" ")
}

fn shifts(a: &V, b: &V) -> [V; 6] {
    [
        shift(a, b, true, false),
        shift(a, b, false, false),
        shift(a, b, false, true),
        shift(a, b, true, false),
        shift(a, b, false, true),
        shift(a, b, false, false),
    ]
}

/// Lines printed by `arith_matrix.sv` and `arith_matrix_2001.v` for `widths`.
fn matrix_expected(widths: &[usize]) -> String {
    let mut out = String::new();
    for &w in widths {
        for i in 0..OPERANDS {
            let a = operand(w, i);
            let minus = hex(&negate(&a));
            writeln!(out, "{w} {i} n {minus} {minus}").unwrap();
            for j in 0..OPERANDS {
                let b = operand(w, j);
                let signed =
                    [Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod].map(|op| arith(op, &a, &b, true));
                let unsigned = [Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod]
                    .map(|op| arith(op, &a, &b, false));
                writeln!(
                    out,
                    "{w} {i} {j} s {} {} u {} {} m {} {} {} {}",
                    hex_list(&signed),
                    hex(&power(&a, true, &b, true)),
                    hex_list(&unsigned),
                    hex(&power(&a, false, &b, false)),
                    hex(&arith(Op::Div, &a, &b, false)),
                    hex(&arith(Op::Mod, &a, &b, false)),
                    hex(&power(&a, true, &b, false)),
                    hex(&power(&a, false, &b, true)),
                )
                .unwrap();
                let shifted = shifts(&a, &b);
                let text = if i < FIRST_UNKNOWN {
                    hex_list(&shifted)
                } else {
                    shifted.iter().map(bin).collect::<Vec<_>>().join(" ")
                };
                writeln!(out, "{w} {i} {j} sh {text}").unwrap();
            }
        }
    }
    out
}

/// `wide.d()`: low and high 64 bits and the count of one bits.
fn digest(value: &V) -> String {
    format!(
        "{}:{}:{}",
        hex(&slice(value, 0, 64)),
        hex(&slice(value, value.w - 64, 64)),
        count_ones(value)
    )
}

fn digest_list(values: &[V]) -> String {
    values.iter().map(digest).collect::<Vec<_>>().join(" ")
}

fn wide_expected() -> String {
    let mut out = String::new();
    for w in [8128, 8129] {
        for i in 0..OPERANDS {
            let a = operand(w, i);
            writeln!(out, "{w} {i} n {}", digest(&negate(&a))).unwrap();
            for j in 0..OPERANDS {
                let b = operand(w, j);
                let signed =
                    [Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod].map(|op| arith(op, &a, &b, true));
                let unsigned = [Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod]
                    .map(|op| arith(op, &a, &b, false));
                writeln!(
                    out,
                    "{w} {i} {j} s {} u {} m {} {}",
                    digest_list(&signed),
                    digest_list(&unsigned),
                    digest(&arith(Op::Div, &a, &b, false)),
                    digest(&arith(Op::Mod, &a, &b, false)),
                )
                .unwrap();
                if i < FIRST_UNKNOWN {
                    writeln!(
                        out,
                        "{w} {i} {j} sh {} {} {} {}",
                        digest(&shift(&a, &b, true, false)),
                        digest(&shift(&a, &b, false, false)),
                        digest(&shift(&a, &b, false, true)),
                        digest(&shift(&a, &b, false, true)),
                    )
                    .unwrap();
                }
                // Mirrors the fixture's selection of affordable powers.
                let odd_base = matches!(i, 3 | 6 | 7 | 8 | 10);
                if !odd_base || j <= 3 || j >= FIRST_UNKNOWN {
                    writeln!(
                        out,
                        "{w} {i} {j} p {} {}",
                        digest(&power(&a, true, &b, true)),
                        digest(&power(&a, false, &b, false)),
                    )
                    .unwrap();
                } else if !matches!(j, 6 | 9) {
                    writeln!(out, "{w} {i} {j} p {}", digest(&power(&a, true, &b, true))).unwrap();
                }
            }
        }
        let odd = power(&operand(w, 3), false, &operand(w, 6), false);
        writeln!(out, "{w} odd {}", digest(&odd)).unwrap();
    }
    out
}

/// Assignment-context binary operation (SV 11.8.2): both operands extend to
/// max(WA, WB, WR), signed only when both are signed, then truncate to WR.
fn context_binary(op: Op, a: &V, a_signed: bool, b: &V, b_signed: bool, wr: usize) -> V {
    let width = a.w.max(b.w).max(wr);
    let signed = a_signed && b_signed;
    arith(
        op,
        &a.resize(width, signed),
        &b.resize(width, signed),
        signed,
    )
    .resize(wr, false)
}

fn mixed_expected() -> String {
    let mut out = String::new();
    let shapes = [
        (31, 65, 129),
        (64, 63, 129),
        (129, 32, 64),
        (65, 1, 33),
        (32, 32, 65),
        (1, 129, 127),
    ];
    for (wa, wb, wr) in shapes {
        for i in 0..OPERANDS {
            let a = operand(wa, i);
            for j in 0..OPERANDS {
                let b = operand(wb, j);
                let x = [
                    context_binary(Op::Add, &a, true, &b, true, wr),
                    context_binary(Op::Sub, &a, true, &b, true, wr),
                    context_binary(Op::Mul, &a, true, &b, true, wr),
                    context_binary(Op::Div, &a, true, &b, true, wr),
                    context_binary(Op::Mod, &a, true, &b, true, wr),
                    context_binary(Op::Add, &a, true, &b, false, wr),
                    context_binary(Op::Mul, &a, true, &b, false, wr),
                    context_binary(Op::Div, &a, true, &b, false, wr),
                    context_binary(Op::Mod, &a, true, &b, false, wr),
                    context_binary(Op::Sub, &a, false, &b, true, wr),
                ];
                // The base alone sizes and signs `**`, `>>>` and `<<`; the
                // exponent and shift count stay self-determined.
                let width = wa.max(wr);
                let signed_base = a.resize(width, true);
                let unsigned_base = a.resize(width, false);
                let p = [
                    power(&signed_base, true, &b, true).resize(wr, false),
                    power(&unsigned_base, false, &b, true).resize(wr, false),
                ];
                writeln!(
                    out,
                    "{wa} {wb} {wr} {i} {j} x {} p {}",
                    hex_list(&x),
                    hex_list(&p)
                )
                .unwrap();
                if i < FIRST_UNKNOWN {
                    let sh = [
                        shift(&signed_base, &b, false, true).resize(wr, false),
                        shift(&unsigned_base, &b, true, false).resize(wr, false),
                    ];
                    writeln!(out, "{wa} {wb} {wr} {i} {j} sh {}", hex_list(&sh)).unwrap();
                }
            }
        }
    }
    out
}

fn constants_expected() -> String {
    let mut out = String::new();
    let indices = [0, 1, 4, 5, 6, 10, 11];
    for w in [1, 32, 64, 65] {
        for i in indices {
            let a = operand(w, i);
            for j in indices {
                let b = operand(w, j);
                let k = [
                    negate(&a),
                    arith(Op::Add, &a, &b, true),
                    arith(Op::Sub, &a, &b, true),
                    arith(Op::Mul, &a, &b, true),
                    arith(Op::Div, &a, &b, true),
                    arith(Op::Mod, &a, &b, true),
                    power(&a, true, &b, true),
                    arith(Op::Mul, &a, &b, false),
                    arith(Op::Div, &a, &b, false),
                    arith(Op::Mod, &a, &b, false),
                    power(&a, false, &b, false),
                    arith(Op::Div, &a, &b, false),
                    power(&a, true, &b, false),
                    power(&a, false, &b, true),
                ];
                writeln!(out, "{w} {i} {j} k {}", hex_list(&k)).unwrap();
                if i != 11 {
                    let ks = [
                        shift(&a, &b, true, false),
                        shift(&a, &b, false, false),
                        shift(&a, &b, false, true),
                        shift(&a, &b, false, true),
                    ];
                    writeln!(out, "{w} {i} {j} ks {}", hex_list(&ks)).unwrap();
                }
            }
        }
    }
    out
}

#[test]
fn arithmetic_matrix_matches_independent_oracle() {
    let expected = matrix_expected(&[1, 31, 32, 63, 64, 65, 127, 128, 129]);
    sim_cli::run_case(SUITE, "arith_matrix", &expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "arith_matrix", &expected, &[], &[]);
}

#[test]
fn arithmetic_matrix_2001_matches_independent_oracle() {
    let expected = matrix_expected(&[1, 31, 32, 63, 64, 65, 129]);
    sim_cli::run_case_with_args(
        SUITE,
        "arith_matrix_2001.v",
        &expected,
        "",
        &[],
        &["--edition", "2001"],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "arith_matrix_2001.v",
        &expected,
        &["--edition", "2001"],
        &[],
    );
}

#[test]
fn arithmetic_at_kernel_threshold_widths_matches_independent_oracle() {
    let expected = wide_expected();
    sim_cli::run_case(SUITE, "arith_wide", &expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "arith_wide", &expected, &[], &[]);
}

#[test]
fn mixed_width_contexts_match_independent_oracle() {
    let expected = mixed_expected();
    sim_cli::run_case(SUITE, "arith_mixed", &expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "arith_mixed", &expected, &[], &[]);
}

#[test]
fn constant_operands_fold_like_runtime_operations() {
    // The default optimizer folds literal operands it can evaluate; --no-opt
    // computes every one at run time. Both must match the same oracle.
    let expected = constants_expected();
    sim_cli::run_case(SUITE, "arith_constants", &expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "arith_constants", &expected, &[], &[]);
}

#[test]
fn mutations_evaluate_receivers_indices_and_calls_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_006/mutation_contexts.out");
    sim_cli::run_case(SUITE, "mutation_contexts", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "mutation_contexts", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "mutation_contexts", expected);
}

#[test]
fn neg_continuous_increment() {
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_increment",
        "increment and decrement expressions are not allowed in this context",
    );
}

#[test]
fn neg_port_compound() {
    sim_cli::reject_case(
        SUITE,
        "neg_port_compound",
        "assignment expressions are not allowed in this context",
    );
}

#[test]
fn neg_call_result_increment() {
    sim_cli::reject_case(
        SUITE,
        "neg_call_result_increment",
        "expression is not assignable",
    );
}

#[test]
fn neg_compound_2001() {
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_compound_2001.v",
        "`+=` is not available in IEEE 2001",
        &["--edition", "2001"],
    );
}
