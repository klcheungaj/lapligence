//! SIM-037: single-clock sequence composition (IEEE 1800-2009 §§16.7-16.10,
//! Annex F). A01 compares the public CLI against a test-side trace
//! interpreter written from the Annex F tight-satisfaction rules; the other
//! oracles are derived by hand in the fixture readme.
use super::sim_cli;
use std::collections::BTreeMap;

const SUITE: &str = "feature_completion/sim_037";

// ---------------------------------------------------------------------------
// Annex F trace interpreter (test side only; shares nothing with the NFA).
//
// `matches(s, w, i)` maps each position `j` such that `s` tightly satisfies
// the finite word `w[i..=j]` to the number of distinct matches ending there;
// `j == i - 1` is the empty word.
// Letters past the end of `w` do not exist, so no atom matches there.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Atom {
    A,
    B,
    NotA,
    NotB,
    True,
}

impl Atom {
    fn holds(self, letter: (bool, bool)) -> bool {
        match self {
            Atom::A => letter.0,
            Atom::B => letter.1,
            Atom::NotA => !letter.0,
            Atom::NotB => !letter.1,
            Atom::True => true,
        }
    }

    fn text(self) -> &'static str {
        match self {
            Atom::A => "a",
            Atom::B => "b",
            Atom::NotA => "!a",
            Atom::NotB => "!b",
            Atom::True => "1",
        }
    }
}

type Bound = Option<u32>;

enum Seq {
    Bool(Atom),
    /// `l ##[m:n] r`.
    Cat(Box<Seq>, u32, Bound, Box<Seq>),
    /// Leading `##[m:n] r`, defined as `1[*m:n] ##1 r` (F.3.4.2.2).
    Lead(u32, Bound, Box<Seq>),
    /// Consecutive repetition `(s)[*m:n]`.
    Rep(Box<Seq>, u32, Bound),
    /// `b[->m:n]` = `(!b[*0:$] ##1 b)[*m:n]`.
    Goto(Atom, u32, Bound),
    /// `b[=m:n]` = `b[->m:n] ##1 !b[*0:$]`.
    Noncons(Atom, u32, Bound),
    Or(Box<Seq>, Box<Seq>),
    And(Box<Seq>, Box<Seq>),
    Intersect(Box<Seq>, Box<Seq>),
    /// `e throughout s` = `e[*0:$] intersect s`.
    Throughout(Atom, Box<Seq>),
    /// `s1 within s2` = `(1[*0:$] ##1 s1 ##1 1[*0:$]) intersect s2`.
    Within(Box<Seq>, Box<Seq>),
    FirstMatch(Box<Seq>),
}

use Seq::*;

fn b(atom: Atom) -> Box<Seq> {
    Box::new(Bool(atom))
}

fn bx(seq: Seq) -> Box<Seq> {
    Box::new(seq)
}

fn negate(atom: Atom) -> Atom {
    match atom {
        Atom::A => Atom::NotA,
        Atom::B => Atom::NotB,
        Atom::NotA => Atom::A,
        Atom::NotB => Atom::B,
        Atom::True => panic!("goto and nonconsecutive operands are a or b"),
    }
}

type Word = [(bool, bool)];

/// Match ends with their multiplicity (number of distinct matches).
type Ends = BTreeMap<i64, u64>;

fn add(out: &mut Ends, end: i64, count: u64) {
    if count > 0 {
        *out.entry(end).or_insert(0) += count;
    }
}

fn add_all(out: &mut Ends, ends: &Ends, factor: u64) {
    for (end, count) in ends {
        add(out, *end, count * factor);
    }
}

/// Matches of `seq` started at position `i`. Every distinct way of matching
/// counts (§16.9.5 pairs every operand match, §16.9.7 counts each `or`
/// operand, §16.9.8 keeps all earliest matches); an empty match consumes no
/// clock tick, so all ways of matching the empty word are one match.
fn matches(seq: &Seq, w: &Word, i: i64) -> Ends {
    let mut out = matches_raw(seq, w, i);
    if let Some(empty) = out.get_mut(&(i - 1)) {
        *empty = 1;
    }
    out
}

fn matches_raw(seq: &Seq, w: &Word, i: i64) -> Ends {
    let len = w.len() as i64;
    match seq {
        Bool(atom) => {
            let mut out = Ends::new();
            if i < len && atom.holds(w[i as usize]) {
                add(&mut out, i, 1);
            }
            out
        }
        Cat(left, min, max, right) => {
            let mut out = Ends::new();
            for (j, count) in matches(left, w, i) {
                add_all(&mut out, &delayed(*min, *max, right, w, j, j >= i), count);
            }
            out
        }
        Lead(min, max, right) => {
            // `1[*k] ##1 r` for each k in [m:n]: k true letters starting at
            // i, which must exist, then `r` from i + k (with k = 0 this is `r`
            // itself, including its empty match). Each k is a distinct way.
            let mut out = Ends::new();
            let top = max.map_or(len + 1, i64::from);
            for k in i64::from(*min)..=top {
                if k > 0 && i + k - 1 >= len {
                    break;
                }
                add_all(&mut out, &matches(right, w, i + k), 1);
            }
            out
        }
        Rep(body, min, max) => repeat(&|start| matches(body, w, start), *min, *max, i, len),
        Goto(atom, min, max) => {
            let one = |start: i64| {
                let skip = Cat(bx(Rep(b(negate(*atom)), 0, None)), 1, Some(1), b(*atom));
                matches(&skip, w, start)
            };
            repeat(&one, *min, *max, i, len)
        }
        Noncons(atom, min, max) => {
            let mut out = Ends::new();
            for (j, count) in matches(&Goto(*atom, *min, *max), w, i) {
                // `##1 !b[*0:$]` after the last occurrence.
                let tail = repeat(
                    &|start| matches(&Bool(negate(*atom)), w, start),
                    0,
                    None,
                    j + 1,
                    len,
                );
                add_all(&mut out, &tail, count);
            }
            out
        }
        Or(left, right) => {
            let mut out = matches(left, w, i);
            add_all(&mut out, &matches(right, w, i), 1);
            out
        }
        And(left, right) => {
            let right = matches(right, w, i);
            let mut out = Ends::new();
            for (l, lc) in matches(left, w, i) {
                for (r, rc) in &right {
                    add(&mut out, l.max(*r), lc * rc);
                }
            }
            out
        }
        Intersect(left, right) => intersect(&matches(left, w, i), &matches(right, w, i)),
        Throughout(atom, right) => {
            let left = repeat(&|start| matches(&Bool(*atom), w, start), 0, None, i, len);
            intersect(&left, &matches(right, w, i))
        }
        Within(inner, outer) => {
            let pad =
                |start: i64| repeat(&|p| matches(&Bool(Atom::True), w, p), 0, None, start, len);
            let mut left = Ends::new();
            for (lead_end, lead) in pad(i) {
                for (inner_end, count) in matches(inner, w, lead_end + 1) {
                    add_all(&mut left, &pad(inner_end + 1), lead * count);
                }
            }
            intersect(&left, &matches(outer, w, i))
        }
        FirstMatch(inner) => matches(inner, w, i).into_iter().take(1).collect(),
    }
}

/// Pairs of equal ends; each pair is one match.
fn intersect(left: &Ends, right: &Ends) -> Ends {
    let mut out = Ends::new();
    for (end, count) in left {
        if let Some(other) = right.get(end) {
            add(&mut out, *end, count * other);
        }
    }
    out
}

/// Matches of `right` after a left endpoint `j` with `##[min:max]`. `##0`
/// fuses with a nonempty left match only and needs a nonempty right match;
/// `##k` (k >= 1) pads `k - 1` true letters, which must exist. Each delay is
/// a distinct way.
fn delayed(min: u32, max: Bound, right: &Seq, w: &Word, j: i64, fuse: bool) -> Ends {
    let len = w.len() as i64;
    let mut out = Ends::new();
    let top = max.map_or(len + 1, i64::from);
    for delay in i64::from(min)..=top {
        if delay == 0 {
            if fuse {
                for (end, count) in matches(right, w, j) {
                    if end >= j {
                        add(&mut out, end, count);
                    }
                }
            }
            continue;
        }
        let start = j + delay;
        if start > len {
            break;
        }
        add_all(&mut out, &matches(right, w, start), 1);
    }
    out
}

/// `R[*m:n]` with `R[*0]` = empty and `R[*k+1]` = `R[*k] ##1 R`. A match is a
/// chain of k nonempty matches of `R`; when `R` admits the empty match, the
/// remaining iterations are empty and consume no clock tick, so they add no
/// distinct match (and `[*0:$]` stays finite). Such a chain counts for any k
/// up to `n`; otherwise k must also reach `m`.
fn repeat(one: &dyn Fn(i64) -> Ends, min: u32, max: Bound, i: i64, len: i64) -> Ends {
    let body_empty = one(i).contains_key(&(i - 1));
    let mut out = Ends::new();
    if min == 0 || body_empty {
        add(&mut out, i - 1, 1);
    }
    let mut current = Ends::from([(i - 1, 1)]);
    let mut count = 0u32;
    while !current.is_empty() && max.is_none_or(|max| count < max) {
        let mut next = Ends::new();
        for (end, ways) in &current {
            if *end >= len {
                continue;
            }
            for (e, c) in one(end + 1) {
                if e > *end {
                    add(&mut next, e, ways * c);
                }
            }
        }
        count += 1;
        if count >= min || body_empty {
            add_all(&mut out, &next, 1);
        }
        current = next;
    }
    out
}

fn range_text(min: u32, max: Bound) -> String {
    match max {
        None => format!("{min}:$"),
        Some(max) if max == min => format!("{min}"),
        Some(max) => format!("{min}:{max}"),
    }
}

fn delay_text(min: u32, max: Bound) -> String {
    if max == Some(min) {
        format!("{min}")
    } else {
        format!("[{}]", range_text(min, max))
    }
}

/// SystemVerilog text of `seq`; the fixture must contain exactly this text.
fn render(seq: &Seq) -> String {
    match seq {
        Bool(atom) => atom.text().to_owned(),
        Cat(l, min, max, r) => {
            format!("({} ##{} {})", render(l), delay_text(*min, *max), render(r))
        }
        Lead(min, max, r) => format!("(##{} {})", delay_text(*min, *max), render(r)),
        Rep(body, min, max) => format!("({})[*{}]", render(body), range_text(*min, *max)),
        Goto(atom, min, max) => format!("{}[->{}]", atom.text(), range_text(*min, *max)),
        Noncons(atom, min, max) => format!("{}[={}]", atom.text(), range_text(*min, *max)),
        Or(l, r) => format!("({} or {})", render(l), render(r)),
        And(l, r) => format!("({} and {})", render(l), render(r)),
        Intersect(l, r) => format!("({} intersect {})", render(l), render(r)),
        Throughout(atom, r) => format!("({} throughout {})", atom.text(), render(r)),
        Within(l, r) => format!("({} within {})", render(l), render(r)),
        FirstMatch(inner) => format!("first_match({})", render(inner)),
    }
}

/// The bounded and unbounded sequences of `exhaustive.sv`, in label order.
fn exhaustive_sequences() -> Vec<Seq> {
    use Atom::{True, A, B};
    let cat = |l: Box<Seq>, min, max, r: Box<Seq>| Cat(l, min, max, r);
    let rep = |s: Box<Seq>, min, max| bx(Rep(s, min, max));
    vec![
        Bool(A),
        cat(b(A), 1, Some(1), b(B)),
        cat(b(A), 0, Some(0), b(B)),
        cat(b(A), 1, Some(3), b(B)),
        cat(b(A), 2, None, b(B)),
        Lead(1, Some(2), b(B)),
        cat(rep(b(A), 0, Some(0)), 1, Some(1), b(B)),
        cat(rep(b(A), 0, Some(2)), 0, Some(0), b(B)),
        Rep(b(A), 1, Some(3)),
        Rep(b(A), 0, None),
        Rep(bx(cat(b(A), 1, Some(1), b(B))), 1, Some(2)),
        Rep(bx(cat(rep(b(A), 0, Some(1)), 1, Some(1), b(B))), 2, None),
        Rep(bx(Or(rep(b(A), 0, Some(0)), b(B))), 2, Some(3)),
        Goto(B, 2, Some(2)),
        cat(bx(Goto(A, 1, Some(2))), 1, Some(1), b(B)),
        cat(bx(Noncons(B, 1, Some(1))), 1, Some(1), b(A)),
        Noncons(A, 0, Some(2)),
        Or(bx(cat(b(A), 1, Some(1), b(B))), rep(b(B), 2, Some(2))),
        And(bx(cat(b(A), 1, Some(2), b(B))), rep(b(B), 1, Some(3))),
        And(rep(b(A), 0, Some(1)), bx(cat(b(B), 1, Some(1), b(B)))),
        And(rep(b(A), 0, Some(2)), rep(b(B), 0, Some(1))),
        Intersect(rep(b(A), 1, Some(4)), bx(cat(b(B), 1, None, b(A)))),
        Intersect(
            bx(cat(rep(b(True), 0, Some(3)), 1, Some(1), b(B))),
            rep(b(A), 2, Some(3)),
        ),
        Intersect(rep(b(A), 0, Some(2)), rep(b(B), 0, Some(2))),
        Throughout(A, bx(cat(b(B), 1, Some(2), b(B)))),
        Throughout(Atom::NotB, rep(b(A), 0, Some(2))),
        Within(b(B), bx(cat(b(A), 2, Some(3), b(A)))),
        Within(bx(cat(b(A), 1, Some(1), b(A))), rep(b(True), 1, Some(4))),
        FirstMatch(bx(cat(b(A), 1, Some(3), b(B)))),
        FirstMatch(bx(Or(
            rep(b(A), 1, Some(3)),
            bx(cat(b(B), 1, Some(1), b(B))),
        ))),
        FirstMatch(bx(And(rep(b(A), 1, None), bx(cat(b(B), 1, Some(2), b(A)))))),
        Or(
            bx(Intersect(rep(b(A), 1, Some(2)), rep(b(B), 1, Some(2)))),
            bx(FirstMatch(bx(Within(b(A), rep(b(B), 2, Some(3)))))),
        ),
        cat(
            bx(And(b(A), bx(cat(b(B), 1, Some(1), b(B))))),
            1,
            Some(1),
            bx(Intersect(
                rep(b(A), 1, Some(2)),
                bx(cat(b(B), 0, Some(1), b(A))),
            )),
        ),
        Rep(bx(And(b(A), bx(cat(b(B), 1, Some(1), b(A))))), 1, Some(2)),
        cat(rep(b(A), 0, Some(1)), 1, Some(1), rep(b(B), 0, Some(1))),
        Lead(0, Some(1), rep(b(A), 0, Some(1))),
    ]
}

const TRACE_LENGTH: usize = 5;

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

#[test]
fn bounded_sequences_match_the_annex_f_interpreter_on_every_trace() {
    let sequences = exhaustive_sequences();
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join("exhaustive.sv"),
    )
    .expect("exhaustive fixture");
    let mut expected = Vec::new();
    for (index, seq) in sequences.iter().enumerate() {
        let line = format!(
            "  s{index:02}: cover sequence (@(posedge clk) disable iff (kill) go ##1 {}) $display(\"{index:02} %0d %0d\", trace, pos);",
            render(seq)
        );
        assert!(
            source.contains(&line),
            "fixture lacks interpreter sequence {index}: {line}"
        );
        for trace in 0..(1u32 << (2 * TRACE_LENGTH)) {
            let word: Vec<(bool, bool)> = (0..TRACE_LENGTH)
                .map(|k| ((trace >> (2 * k)) & 1 == 1, (trace >> (2 * k + 1)) & 1 == 1))
                .collect();
            // Position p of the word is tick p + 1; an empty match ends at
            // the `go` tick 0.
            for (end, count) in matches(seq, &word, 0) {
                for _ in 0..count {
                    expected.push(format!("{index:02} {trace} {}", end + 1));
                }
            }
        }
    }
    assert_eq!(
        source.matches(": cover sequence").count(),
        sequences.len(),
        "fixture and interpreter list different sequences"
    );
    expected.sort();
    sim_cli::run_case_checked_matrix(SUITE, "exhaustive", &[], &|label, output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual = sorted_lines(&String::from_utf8_lossy(&output.stdout));
        assert!(
            actual == expected,
            "{label}: {} match lines, interpreter has {}; first difference {:?}",
            actual.len(),
            expected.len(),
            actual.iter().zip(expected.iter()).find(|(a, e)| a != e)
        );
    });
}

fn ends(pairs: &[(i64, u64)]) -> Ends {
    pairs.iter().copied().collect()
}

#[test]
fn interpreter_follows_the_annex_f_empty_match_rules() {
    // Hand-checked anchors for the interpreter itself (16.9.2.1): with
    // `a` true everywhere, `(a[*0] ##0 b)` never matches, `(a[*0] ##1 b)` is
    // `b`, `(b ##1 a[*0])` is `b ##0 1`, and `a[*0:1]` matches empty and at 0.
    let word = [(true, true), (true, false)];
    let empty_fuse = Cat(bx(Rep(b(Atom::A), 0, Some(0))), 0, Some(0), b(Atom::B));
    assert!(matches(&empty_fuse, &word, 0).is_empty());
    let empty_cat = Cat(bx(Rep(b(Atom::A), 0, Some(0))), 1, Some(1), b(Atom::B));
    assert_eq!(matches(&empty_cat, &word, 0), ends(&[(0, 1)]));
    let trailing = Cat(b(Atom::B), 1, Some(1), bx(Rep(b(Atom::A), 0, Some(0))));
    assert_eq!(matches(&trailing, &word, 0), ends(&[(0, 1)]));
    assert_eq!(
        matches(&Rep(b(Atom::A), 0, Some(1)), &word, 0),
        ends(&[(-1, 1), (0, 1)])
    );
    // `and` ends at the later endpoint; `intersect` needs equal endpoints.
    let and = And(b(Atom::A), bx(Rep(b(Atom::A), 1, Some(2))));
    assert_eq!(matches(&and, &word, 0), ends(&[(0, 1), (1, 1)]));
    let intersect = Intersect(b(Atom::A), bx(Rep(b(Atom::A), 2, Some(2))));
    assert!(matches(&intersect, &word, 0).is_empty());
    // `##[0:1] r` = `r or (1 ##1 r)` keeps the empty match of `r` like
    // `##0 r` (F.3.4.2.2): `##[0:1] b[*0:1]` ends at -1 (empty) and twice at
    // 0 (`b` on letter 0, and `1` followed by the empty `b[*0]`).
    let lead = Lead(0, Some(1), bx(Rep(b(Atom::B), 0, Some(1))));
    assert_eq!(matches(&lead, &word, 0), ends(&[(-1, 1), (0, 2)]));
}

#[test]
fn interpreter_counts_each_distinct_match() {
    let word = [(true, true), (true, false)];
    // 16.9.7: each `or` operand match is a match of the composite.
    let or = Or(
        bx(Cat(b(Atom::A), 1, Some(1), b(Atom::A))),
        bx(Cat(b(Atom::B), 1, Some(1), b(Atom::A))),
    );
    assert_eq!(matches(&or, &word, 0), ends(&[(1, 2)]));
    // 16.9.5: every pair of operand matches is a match; the empty left match
    // and the match at 0 both pair with the right match at 1.
    let and = And(
        bx(Rep(b(Atom::A), 0, Some(1))),
        bx(Cat(b(Atom::B), 1, Some(1), b(Atom::A))),
    );
    assert_eq!(matches(&and, &word, 0), ends(&[(1, 2)]));
    // 16.9.8: first_match keeps every match at the earliest end.
    assert_eq!(matches(&FirstMatch(bx(or)), &word, 0), ends(&[(1, 2)]));
    // Empty iterations of an empty-admitting body add no distinct match.
    let rep = Rep(bx(Rep(b(Atom::A), 0, Some(1))), 1, Some(2));
    assert_eq!(matches(&rep, &word, 0), ends(&[(-1, 1), (0, 1), (1, 1)]));
}

fn assert_sorted_output(label: &str, output: &std::process::Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        sorted_lines(&String::from_utf8_lossy(&output.stdout)),
        sorted_lines(expected),
        "{label}: match lines differ from the hand-derived set"
    );
}

#[test]
fn match_sets_keep_multiplicity_and_endpoint_alignment() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_037/match_sets.out");
    sim_cli::run_case_checked_matrix(SUITE, "match_sets", &[], &|label, output| {
        assert_sorted_output(label, output, expected)
    });
}

#[test]
fn unbounded_obligations_stay_pending_until_met_or_impossible() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_037/unbounded.out");
    sim_cli::run_case_checked_matrix(SUITE, "unbounded", &[], &|label, output| {
        assert_sorted_output(label, output, expected)
    });
}

#[test]
fn sequence_thread_budget_reports_exhaustion() {
    sim_cli::run_case_checked_matrix(SUITE, "budget", &[], &|label, output| {
        assert_sorted_output(label, output, "done t=200\n")
    });
    for optimized in [false, true] {
        let limited = sim_cli::invoke_with_env(
            SUITE,
            "budget",
            optimized,
            &[],
            &[("LLG_SEQUENCE_THREAD_LIMIT", "50")],
            &[],
        );
        let stderr = String::from_utf8_lossy(&limited.stderr);
        assert!(!limited.status.success(), "optimized={optimized}: {stderr}");
        assert!(
            stderr.contains(
                "llg: sequence thread budget exhausted: more than 50 live sequence threads at time "
            ) && stderr.contains("(concurrent assertion p at "),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            !String::from_utf8_lossy(&limited.stdout).contains("done"),
            "optimized={optimized}: the run must stop at the budget"
        );
        let invalid = sim_cli::invoke_with_env(
            SUITE,
            "budget",
            optimized,
            &[],
            &[("LLG_SEQUENCE_THREAD_LIMIT", "0")],
            &[],
        );
        let stderr = String::from_utf8_lossy(&invalid.stderr);
        assert!(!invalid.status.success(), "optimized={optimized}: {stderr}");
        assert!(
            stderr.contains(
                "llg: invalid LLG_SEQUENCE_THREAD_LIMIT (must be a positive decimal uint64)"
            ),
            "optimized={optimized}: {stderr}"
        );
    }
}

#[test]
fn match_items_run_once_per_counted_path() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_037/multiplicity_paths.out");
    sim_cli::run_case_checked_matrix(SUITE, "multiplicity_paths", &[], &|label, output| {
        assert_sorted_output(label, output, expected)
    });
}

#[test]
fn match_multiplicity_limits_are_reported_errors() {
    sim_cli::run_case_checked_matrix(SUITE, "multiplicity_budget", &[], &|label, output| {
        assert_sorted_output(label, output, "done hits=4096\n")
    });
    for optimized in [false, true] {
        let limited = sim_cli::invoke_with_env(
            SUITE,
            "multiplicity_budget",
            optimized,
            &[],
            &[("LLG_SEQUENCE_THREAD_LIMIT", "50")],
            &[],
        );
        let stderr = String::from_utf8_lossy(&limited.stderr);
        assert!(!limited.status.success(), "optimized={optimized}: {stderr}");
        assert!(
            stderr.contains(
                "llg: sequence thread budget exhausted: 4096 matches must each run match items or a pass statement at time "
            ) && stderr.contains("(concurrent assertion p at "),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            !String::from_utf8_lossy(&limited.stdout).contains("done"),
            "optimized={optimized}: the run must stop at the budget"
        );
        let overflow =
            sim_cli::invoke_with_env(SUITE, "multiplicity_overflow", optimized, &[], &[], &[]);
        let stderr = String::from_utf8_lossy(&overflow.stderr);
        assert!(
            !overflow.status.success(),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            stderr.contains("llg: sequence match multiplicity overflow at time 635000 (concurrent assertion p at "),
            "optimized={optimized}: {stderr}"
        );
        assert!(
            !String::from_utf8_lossy(&overflow.stdout).contains("done"),
            "optimized={optimized}: the run must stop at the overflow"
        );
    }
}

// Forms owned by other tasks or illegal by the LRM are diagnosed, never
// approximated.

#[test]
fn neg_join_local() {
    sim_cli::reject_case(
        SUITE,
        "neg_join_local",
        "local variable assignments inside `and` operands are not supported",
    );
}

#[test]
fn neg_property_and() {
    sim_cli::reject_case(
        SUITE,
        "neg_property_and",
        "assertion binary operator OverlappedImplication is not supported",
    );
}

#[test]
fn neg_multiclock_and() {
    sim_cli::reject_case(
        SUITE,
        "neg_multiclock_and",
        "assertion expression does not have a unique leading clock",
    );
}

#[test]
fn neg_goto_sequence() {
    sim_cli::reject_case(
        SUITE,
        "neg_goto_sequence",
        "sequences cannot specify a non-consecutive or go-to repetition",
    );
}

#[test]
fn neg_bad_clock_witness() {
    // FND-002 L-F12-10-03 witness `neg_assert_bad_clock`, adopted unchanged.
    sim_cli::reject_case(
        SUITE,
        "neg_bad_clock_witness",
        "multiclocked sequence operands cannot be combined with any sequence operators other than ##1 and ##0",
    );
}
