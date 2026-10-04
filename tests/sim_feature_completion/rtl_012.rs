//! RTL-012: non-charge strength resolution, strength observation and
//! `unconnected_drive`.
//!
//! The scalar matrix and the wide composition are checked against the
//! exhaustive outcome oracle in `rtl_012/oracle.rs`, never against simulator
//! output; the other `.out` files are hand-derived from the cited clauses.

use super::sim_cli;
use std::fmt::Write;

#[path = "rtl_012/oracle.rs"]
mod oracle;

use oracle::{bit, resolve, strength_text, value_text, Bit, Net, Source};

const SUITE: &str = "feature_completion/rtl_012";

/// Driver sources of `scalar_matrix.v`, in fixture order: continuous
/// assignments with `(strength0, strength1)`, then `bufif1` gates with an
/// unknown enable.
const SOURCES: [(bool, i8, i8); 10] = [
    (false, 6, 6),
    (false, 7, 7),
    (false, 5, 5),
    (false, 3, 3),
    (false, 6, 3),
    (false, 3, 5),
    (false, 0, 6),
    (false, 5, 0),
    (true, 6, 6),
    (true, 3, 5),
];

const NETS: [(&str, Net); 5] = [
    ("wire", Net::Wire),
    ("wand", Net::Wand),
    ("wor", Net::Wor),
    ("tri0", Net::Tri0),
    ("tri1", Net::Tri1),
];

fn matrix_source(index: usize, value: Bit) -> Source {
    let (gate, s0, s1) = SOURCES[index];
    if gate {
        Source::Bufif1 {
            s0,
            s1,
            data: value,
            enable: Bit::X,
        }
    } else {
        Source::Drive { s0, s1, value }
    }
}

fn matrix_expected() -> String {
    let pairs = (0..SOURCES.len())
        .flat_map(|i| (i..SOURCES.len()).map(move |j| (i, j)))
        .collect::<Vec<_>>();
    let mut out = String::new();
    for a in 0..4 {
        for b in 0..4 {
            for (name, net) in NETS {
                write!(out, "{name} {a} {b}").unwrap();
                for (i, j) in &pairs {
                    let sources = [matrix_source(*i, bit(a)), matrix_source(*j, bit(b))];
                    write!(out, " {}", strength_text(resolve(net, &sources))).unwrap();
                }
                out.push('\n');
            }
        }
    }
    out
}

#[test]
fn scalar_strength_matrix_matches_exhaustive_oracle() {
    let expected = matrix_expected();
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "scalar_matrix.v",
            &expected,
            "",
            &[],
            &["--edition", edition],
        );
    }
    sim_cli::run_case_backend_parity(SUITE, "scalar_matrix.v", &expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "scalar_matrix.v", &expected);
}

/// Lines printed by `wide_composition.sv`: per step, `%b` and `%v` of the
/// omitted pull1 input `bus` with a `bufif1 (strong0, weak1)` array, the
/// omitted wand input `wbus` with `pulldown (weak0)` and `buf (pull0, pull1)`
/// arrays, and the plain wire `p` with two default-strength drivers.
fn composition_expected() -> String {
    const W: usize = 70;
    let pull1 = Source::Drive {
        s0: 0,
        s1: 5,
        value: Bit::One,
    };
    let mut out = String::new();
    for step in 0..3 {
        let mut nets: [Vec<(i8, i8)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for k in (0..W).rev() {
            let d = bit(k + step);
            let e = bit(k / 4 + step);
            let f = bit(k / 16 + 2 * step);
            nets[0].push(resolve(
                Net::Wire,
                &[
                    pull1,
                    Source::Bufif1 {
                        s0: 6,
                        s1: 3,
                        data: d,
                        enable: e,
                    },
                ],
            ));
            let buffered = if d == Bit::Z { Bit::X } else { d };
            nets[1].push(resolve(
                Net::Wand,
                &[
                    pull1,
                    Source::Drive {
                        s0: 3,
                        s1: 0,
                        value: Bit::Zero,
                    },
                    Source::Drive {
                        s0: 5,
                        s1: 5,
                        value: buffered,
                    },
                ],
            ));
            nets[2].push(resolve(
                Net::Wire,
                &[
                    Source::Drive {
                        s0: 6,
                        s1: 6,
                        value: d,
                    },
                    Source::Drive {
                        s0: 6,
                        s1: 6,
                        value: f,
                    },
                ],
            ));
        }
        for (name, bits) in ["bus", "wbus", "p"].iter().zip(&nets) {
            let values = bits.iter().map(|bit| value_text(*bit)).collect::<String>();
            let strengths = bits
                .iter()
                .map(|bit| strength_text(*bit))
                .collect::<Vec<_>>()
                .join(" ");
            writeln!(out, "{name} {values}").unwrap();
            writeln!(out, "{name} {strengths}").unwrap();
        }
    }
    out
}

#[test]
fn wide_nets_compose_omitted_inputs_gate_arrays_and_competing_drivers() {
    let expected = composition_expected();
    sim_cli::run_case(SUITE, "wide_composition", &expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "wide_composition", &expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "wide_composition", &expected);
}

#[test]
fn unknown_enables_drive_l_and_h() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_012/tristate_lh.out");
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "tristate_lh.v",
            expected,
            "",
            &[],
            &["--edition", edition],
        );
    }
    sim_cli::run_case_backend_parity(SUITE, "tristate_lh.v", expected, &[], &[]);
}

#[test]
fn strength_only_changes_reach_monitors_but_not_value_waiters() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_012/strength_wakeups.out");
    sim_cli::run_case(SUITE, "strength_wakeups", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "strength_wakeups", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "strength_wakeups", expected);
}

#[test]
fn unconnected_drive_follows_directive_lifetime_and_net_type() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_012/directive_lifetime.out");
    let include = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sim/feature_completion/rtl_012"
    );
    sim_cli::run_case_with_args(
        SUITE,
        "directive_lifetime",
        expected,
        "",
        &[],
        &["-I", include],
    );
    sim_cli::run_case_backend_parity(SUITE, "directive_lifetime", expected, &["-I", include], &[]);
}

#[test]
fn unconnected_drive_respects_compilation_units() {
    for (mode, expected) in [
        (
            "separate",
            include_str!("../fixtures/sim/feature_completion/rtl_012/units_separate.out"),
        ),
        (
            "merged",
            include_str!("../fixtures/sim/feature_completion/rtl_012/units_merged.out"),
        ),
    ] {
        sim_cli::run_case_with_source_prefix(
            SUITE,
            "units_main",
            &["units_pull"],
            expected,
            "",
            &[],
            &["--compilation-units", mode],
        );
    }
}

#[test]
fn omitted_and_connected_net_array_formals_resolve_per_cell() {
    let witness = include_str!("../fixtures/sim/feature_completion/rtl_012/unconnected_array.out");
    sim_cli::run_case(SUITE, "unconnected_array", witness, "", &[]);
    sim_cli::run_case_after_db_drop(SUITE, "unconnected_array", witness);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_012/unconnected_aggregates.out");
    sim_cli::run_case(SUITE, "unconnected_aggregates", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "unconnected_aggregates", expected, &[], &[]);
}

#[test]
fn neg_strength_syntax_keeps_context_prohibitions() {
    let supply = "drive strength on continuous assignment to supply net `s` is not permitted";
    sim_cli::reject_case(SUITE, "neg_supply_strength", supply);
    for edition in ["2001", "2009"] {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(SUITE, "neg_supply_decl_strength.v", supply, &args);
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_highz_pair.v",
            "cannot combine 'highz0' and 'highz1'",
            &args,
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_pullup_strength0.v",
            "invalid strength for 'pullup' gate",
            &args,
        );
    }
    sim_cli::reject_case(
        SUITE,
        "neg_selected_vector_strength",
        "drive strength on non-scalar net `w` is not permitted",
    );
}
