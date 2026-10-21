//! ADV-013: multiclock concurrent assertions (IEEE 1800-2009 16.14, 16.17),
//! every clocking event form, `$global_clock` (14.14),
//! `$inferred_clock`/`$inferred_disable` (16.15.7) and recursive properties
//! (16.13.17, Annex F.7). Hand-derived fixtures compare their action lines
//! as a set (actions of different assertions in one time step have no
//! defined order); every positive case runs in both optimizer modes on the
//! legacy, compact/portable and compact/GMP value backends. A01 compares
//! randomized traces against the Annex F interpreter in `adv_013/oracle.rs`.
use super::sim_cli;
use std::process::Output;

#[path = "adv_013/oracle.rs"]
mod oracle;
use oracle::{
    a, at, b, bxq, c1, c2, cat, fuse, nb, one, qat, sb, Letter, Oracle, Outcome, Program, Q,
};

const SUITE: &str = "feature_completion/adv_013";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_013/",
            $name
        ))
    };
}

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// The run succeeds and its action lines are exactly `expected`, in any
/// order.
fn assert_line_set(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    let stdout = llg::ffi::platform::native_text_to_lf(output.stdout.clone());
    let actual = sorted_lines(&String::from_utf8_lossy(&stdout));
    let expected = sorted_lines(expected);
    if actual != expected {
        let missing: Vec<_> = expected
            .iter()
            .filter(|line| actual.binary_search(line).is_err())
            .collect();
        let extra: Vec<_> = actual
            .iter()
            .filter(|line| expected.binary_search(line).is_err())
            .collect();
        panic!("{label}: missing {missing:?}; unexpected {extra:?}");
    }
}

/// A02 / 16.14.1-16.14.3: ##1 and ##0 between clocks, implications, clock
/// flow across |=> and parentheses, the adjointness examples and
/// juxtaposition, at coincident (30, 90) and separate ticks.
#[test]
fn multiclocked_sequences_and_clock_flow() {
    sim_cli::run_case_checked_matrix(SUITE, "flow", &[], &|label, output| {
        assert_line_set(label, output, fixture!("flow.out"))
    });
}

/// A02 / 16.5, 14.13, 14.14, 16.15.7: value-change, `edge`, `or`-list,
/// clocking-block, bit-select, gated and default event-list clocks tick once
/// per time step; `$global_clock` clocks assertions and event controls;
/// inferred clock and disable defaults resolve at the instance.
#[test]
fn clocking_event_forms_and_clock_resolution_functions() {
    sim_cli::run_case_checked_matrix(SUITE, "clocks", &[], &|label, output| {
        assert_line_set(label, output, fixture!("clocks.out"))
    });
}

/// A02 / 16.13.17, F.7: recursive properties, including the 16.13.17
/// examples, mutual recursion, swapped and constant actuals, two recursive
/// call sites, synchronous aborts, recursion under a second clock and the
/// F.7 dependency example (formerly SIM-038's `neg_recursive_property`).
#[test]
fn recursive_properties() {
    sim_cli::run_case_checked_matrix(SUITE, "recursion", &[], &|label, output| {
        assert_line_set(label, output, fixture!("recursion.out"))
    });
}

/// A03 / 16.10, 16.12, 16.15.1, 16.13.1: overlapping local contexts across
/// clocks, a property local's copy per semantic leading clock, disable iff
/// preempting attempts waiting in either domain, actions after both domains'
/// NBA updates of their time step, and strong versus weak obligations open
/// at $finish.
#[test]
fn locals_disables_actions_and_end_across_domains() {
    sim_cli::run_case_checked_matrix(SUITE, "domains", &[], &|label, output| {
        assert_line_set(label, output, fixture!("domains.out"))
    });
    sim_cli::reject_case(
        SUITE,
        "neg_local_two_clocks",
        "neg_local_two_clocks.sv:9:17 is initialized at the property attempt's start, which this \
         property form does not support",
    );
}

/// A02 negatives: the four 16.13.17 restrictions, and the located limit for
/// recursion through local variable formals.
#[test]
fn illegal_recursive_properties_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_rec_not",
        "neg_rec_not.sv:6:25 recursive property instance cannot have the operators 'not', \
         's_nexttime', 's_eventually', 's_always', 's_until', or 's_until_with' applied to it",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_rec_disable",
        "neg_rec_disable.sv:6:5 recursive properties cannot have disable iff conditions",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_rec_no_advance",
        "neg_rec_no_advance.sv:6:21 recursive property instance must occur after a positive \
         advance in time to avoid infinite recursion",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_rec_args",
        "neg_rec_args.sv:6:24 cannot refer to argument 'x' in an argument to a recursive \
         property unless it is the entire argument expression or bound to a local variable \
         formal argument",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_rec_local_formal",
        "recursive property `count` with local variable formal arguments at",
    );
}

/// A02 negatives: clause-linked rejections of illegal multiclock forms and
/// clock resolution.
#[test]
fn illegal_multiclock_forms_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_mc_repetition",
        "sequence operator repetition combines differently clocked or multiclocked sequence \
         operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_first_match",
        "sequence operator first_match combines differently clocked or multiclocked sequence \
         operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_delay",
        "neg_mc_delay.sv:5:41 multiclocked sequence operands cannot be combined with any \
         sequence operators other than ##1 and ##0",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_empty",
        "neg_mc_empty.sv:5:43 sequence admits an empty match; this is not allowed when \
         concatenating differently clocked sequences",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_leading_clocks",
        "neg_leading_clocks.sv:5:23 assertion expression does not have a unique leading clock",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_inferred_no_clock",
        "neg_inferred_no_clock.sv:6:6 no inferred clock in this context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_global_clock_missing",
        "neg_global_clock_missing.sv:5:25 no global clocking is available in this scope",
    );
}

/// The named properties of `oracle.sv` with their actuals substituted:
/// `ra(b)`, `wu(a, b)`, `oc2(b)`, `ph1`, `ph2` and `ts(a, b)`.
fn oracle_program() -> Program {
    let weak = |b| Q::Weak(sb(b));
    let next = |q| Q::NonImp(sb(one()), bxq(q));
    Program {
        bodies: vec![
            // ra(b): b and (1'b1 |=> ra(b))
            Q::And(bxq(weak(b())), bxq(next(Q::Inst(0)))),
            // wu(a, b): b or (a and (1'b1 |=> wu(a, b)))
            Q::Or(
                bxq(weak(b())),
                bxq(Q::And(bxq(weak(a())), bxq(next(Q::Inst(1))))),
            ),
            // oc2(b): b and (1'b1 |=> @(posedge c2) oc2(b))
            Q::And(bxq(weak(b())), bxq(next(qat(c2(), Q::Inst(2))))),
            // ph1: a |-> (b and (1'b1 |=> ph2))
            Q::Imp(sb(a()), bxq(Q::And(bxq(weak(b())), bxq(next(Q::Inst(4)))))),
            // ph2: b |-> (a and (1'b1 |=> ph1))
            Q::Imp(sb(b()), bxq(Q::And(bxq(weak(a())), bxq(next(Q::Inst(3)))))),
            // ts(a, b): (a |=> (b and ts(a, b))) and (b or (1'b1 |=> ts(a, b)))
            Q::And(
                bxq(Q::NonImp(
                    sb(a()),
                    bxq(Q::And(bxq(weak(b())), bxq(Q::Inst(5)))),
                )),
                bxq(Q::Or(bxq(weak(b())), bxq(next(Q::Inst(5))))),
            ),
        ],
    }
}

/// The properties of `oracle.sv` (each under `@(posedge c1)`), as fixture
/// text and as interpreter input.
fn oracle_properties() -> Vec<(&'static str, Q)> {
    let weak = |b| Q::Weak(sb(b));
    let on_c2 = |b| qat(c2(), Q::Weak(sb(b)));
    vec![
        (
            "a ##1 @(posedge c2) b",
            Q::Weak(cat(sb(a()), at(c2(), sb(b())))),
        ),
        (
            "a ##0 @(posedge c2) b",
            Q::Weak(fuse(sb(a()), at(c2(), sb(b())))),
        ),
        ("a |=> @(posedge c2) b", Q::NonImp(sb(a()), bxq(on_c2(b())))),
        (
            "a |-> @(posedge c2) b ##1 a",
            Q::Imp(sb(a()), bxq(qat(c2(), Q::Weak(cat(sb(b()), sb(a())))))),
        ),
        (
            "a ##1 @(posedge c2) b |=> a",
            Q::NonImp(cat(sb(a()), at(c2(), sb(b()))), bxq(weak(a()))),
        ),
        (
            "(a |=> @(posedge c2) b) and (b |=> a)",
            Q::And(
                bxq(Q::NonImp(sb(a()), bxq(on_c2(b())))),
                bxq(Q::NonImp(sb(b()), bxq(weak(a())))),
            ),
        ),
        (
            "a |=> (@(posedge c2) b) or (@(posedge c1) a ##1 b)",
            Q::NonImp(
                sb(a()),
                bxq(Q::Or(
                    bxq(on_c2(b())),
                    bxq(qat(c1(), Q::Weak(cat(sb(a()), sb(b()))))),
                )),
            ),
        ),
        (
            "not (a ##1 @(posedge c2) b)",
            Q::Not(bxq(Q::Weak(cat(sb(a()), at(c2(), sb(b())))))),
        ),
        (
            "if (a) @(posedge c2) b else @(posedge c2) !b",
            Q::IfElse(a(), bxq(on_c2(b())), bxq(on_c2(nb()))),
        ),
        (
            "a |=> nexttime (@(posedge c2) b)",
            Q::NonImp(sb(a()), bxq(Q::Next(bxq(on_c2(b()))))),
        ),
        (
            "a |=> ((@(posedge c2) b) until a)",
            Q::NonImp(sb(a()), bxq(Q::Until(bxq(on_c2(b())), bxq(weak(a()))))),
        ),
        (
            "a |=> always (@(posedge c2) b)",
            Q::NonImp(sb(a()), bxq(Q::Always(bxq(on_c2(b()))))),
        ),
        (
            "a |=> @(posedge c2) ra(b)",
            Q::NonImp(sb(a()), bxq(qat(c2(), Q::Inst(0)))),
        ),
        ("wu(a, b)", Q::Inst(1)),
        ("a |=> oc2(b)", Q::NonImp(sb(a()), bxq(Q::Inst(2)))),
        ("ph1", Q::Inst(3)),
        ("ts(a, b)", Q::Inst(5)),
        (
            "a |=> strong(@(posedge c2) b ##1 b)",
            Q::NonImp(sb(a()), bxq(Q::Strong(at(c2(), cat(sb(b()), sb(b())))))),
        ),
        (
            "b |-> sync_accept_on(a) (1'b1 |=> @(posedge c2) b)",
            Q::Imp(
                sb(b()),
                bxq(Q::SyncAccept(
                    a(),
                    bxq(Q::NonImp(sb(one()), bxq(on_c2(b())))),
                )),
            ),
        ),
    ]
}

/// Seeds of the `unit` instances of `oracle.sv`.
const ORACLE_SEEDS: [u32; 12] = [
    2654435761, 1013904227, 3668339987, 2027808453, 387276917, 3041712679, 1401181143, 4055616905,
    2415085369, 774553835, 3428989595, 1788458061,
];
const ORACLE_STEPS: u64 = 16;

/// The trace of one `unit`: the letters of the time steps in which a clock
/// ticks, with their times, from the fixture's xorshift32 generator.
fn oracle_trace(seed: u32) -> Vec<(Letter, u64)> {
    let mut s = seed;
    let mut trace = Vec::new();
    for step in 0..ORACLE_STEPS {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        let bit = |n: u32| (s >> n) & 1 == 1;
        let (c1, c2) = (bit(2), bit(3));
        if c1 || c2 {
            let letter = Letter::V {
                a: bit(0),
                b: bit(1),
                c1,
                c2,
            };
            trace.push((letter, 10 * step + 5));
        }
    }
    trace
}

/// A01 / Annex F.5, F.7: the generated model reports exactly what the
/// independent interpreter derives for multiclocked, recursive, strong and
/// weak properties on randomized traces with coincident, separate and
/// missing ticks of two irregular clocks; attempts still pending at
/// `$finish` report nothing (no tested operator is an `s_eventually`,
/// decision A13-D7).
#[test]
fn properties_match_the_annex_f_interpreter_on_random_traces() {
    let source = fixture!("oracle.sv");
    let program = oracle_program();
    let properties = oracle_properties();
    let mut expected = Vec::new();
    for (index, (text, q)) in properties.iter().enumerate() {
        let line = format!(
            "  p{index:02}: assert property (@(posedge c1) {text}) $display(\"P{index:02} %0d %0t\", SEED, $time); else $display(\"F{index:02} %0d %0t\", SEED, $time);"
        );
        assert!(
            source.contains(line.as_str()),
            "fixture lacks interpreter property {index}: {line}"
        );
        let mut oracle = Oracle::new(&program, q);
        for seed in ORACLE_SEEDS {
            assert!(source.contains(&format!("unit #(.SEED(32'd{seed}))")));
            let trace = oracle_trace(seed);
            let letters: Vec<Letter> = trace.iter().map(|(letter, _)| *letter).collect();
            for start in 0..letters.len() {
                if !matches!(letters[start], Letter::V { c1: true, .. }) {
                    continue;
                }
                let line = |kind: char, time: u64| format!("{kind}{index:02} {seed} {time}");
                match oracle.outcome(&letters[start..]) {
                    Outcome::Pass(k) => expected.push(line('P', trace[start + k].1)),
                    Outcome::Fail(k) => expected.push(line('F', trace[start + k].1)),
                    Outcome::Pending => {}
                }
            }
        }
    }
    assert_eq!(
        source.matches(": assert property").count(),
        properties.len(),
        "fixture and interpreter list different properties"
    );
    let expected = expected.join("\n");
    sim_cli::run_case_checked_matrix(SUITE, "oracle", &[], &|label, output| {
        assert_line_set(label, output, &expected)
    });
}
