//! SIM-037: single-clock sequence composition (IEEE 1800-2009 §§16.7-16.10,
//! Annex F). A01 compares the public CLI against a test-side trace
//! interpreter written from the Annex F tight-satisfaction rules; the other
//! oracles are derived by hand in the fixture readme.
use super::sim_cli;
use std::collections::BTreeSet;

const SUITE: &str = "feature_completion/sim_037";

// ---------------------------------------------------------------------------
// Annex F trace interpreter (test side only; shares nothing with the NFA).
//
// `matches(s, w, i)` is the set of positions `j` such that `s` tightly
// satisfies the finite word `w[i..=j]`; `j == i - 1` is the empty word.
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

fn matches(seq: &Seq, w: &Word, i: i64) -> BTreeSet<i64> {
    let len = w.len() as i64;
    match seq {
        Bool(atom) => {
            if i < len && atom.holds(w[i as usize]) {
                BTreeSet::from([i])
            } else {
                BTreeSet::new()
            }
        }
        Cat(left, min, max, right) => {
            let mut out = BTreeSet::new();
            for j in matches(left, w, i) {
                out.extend(delayed(*min, *max, right, w, j, j >= i));
            }
            out
        }
        Lead(min, max, right) => {
            // `1[*k] ##1 r` for each k in [m:n]: k true letters starting at
            // i, which must exist, then `r` from i + k (with k = 0 this is `r`
            // itself, including its empty match).
            let mut out = BTreeSet::new();
            let top = max.map_or(len + 1, i64::from);
            for k in i64::from(*min)..=top {
                if k > 0 && i + k - 1 >= len {
                    break;
                }
                out.extend(matches(right, w, i + k));
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
            let mut out = BTreeSet::new();
            for j in matches(&Goto(*atom, *min, *max), w, i) {
                // `##1 !b[*0:$]` after the last occurrence.
                out.extend(repeat(
                    &|start| matches(&Bool(negate(*atom)), w, start),
                    0,
                    None,
                    j + 1,
                    len,
                ));
            }
            out
        }
        Or(left, right) => {
            let mut out = matches(left, w, i);
            out.extend(matches(right, w, i));
            out
        }
        And(left, right) => {
            let left = matches(left, w, i);
            let right = matches(right, w, i);
            left.iter()
                .flat_map(|l| right.iter().map(move |r| *l.max(r)))
                .collect()
        }
        Intersect(left, right) => matches(left, w, i)
            .intersection(&matches(right, w, i))
            .copied()
            .collect(),
        Throughout(atom, right) => {
            let left = repeat(&|start| matches(&Bool(*atom), w, start), 0, None, i, len);
            left.intersection(&matches(right, w, i)).copied().collect()
        }
        Within(inner, outer) => {
            let pad =
                |start: i64| repeat(&|p| matches(&Bool(Atom::True), w, p), 0, None, start, len);
            let mut left = BTreeSet::new();
            for lead_end in pad(i) {
                for inner_end in matches(inner, w, lead_end + 1) {
                    left.extend(pad(inner_end + 1));
                }
            }
            left.intersection(&matches(outer, w, i)).copied().collect()
        }
        FirstMatch(inner) => matches(inner, w, i).into_iter().take(1).collect(),
    }
}

/// Matches of `right` after a left endpoint `j` with `##[min:max]`. `##0`
/// fuses with a nonempty left match only and needs a nonempty right match;
/// `##k` (k >= 1) pads `k - 1` true letters, which must exist.
fn delayed(min: u32, max: Bound, right: &Seq, w: &Word, j: i64, fuse: bool) -> BTreeSet<i64> {
    let len = w.len() as i64;
    let mut out = BTreeSet::new();
    let top = max.map_or(len + 1, i64::from);
    for delay in i64::from(min)..=top {
        if delay == 0 {
            if fuse {
                out.extend(matches(right, w, j).into_iter().filter(|end| *end >= j));
            }
            continue;
        }
        let start = j + delay;
        if start > len {
            break;
        }
        out.extend(matches(right, w, start));
    }
    out
}

/// `R[*m:n]` with `R[*0]` = empty and `R[*k+1]` = `R[*k] ##1 R`.
fn repeat(
    one: &dyn Fn(i64) -> BTreeSet<i64>,
    min: u32,
    max: Bound,
    i: i64,
    len: i64,
) -> BTreeSet<i64> {
    let mut current = BTreeSet::from([i - 1]);
    let mut out = BTreeSet::new();
    if min == 0 {
        out.extend(current.iter().copied());
    }
    let mut count = 0u32;
    while count < min || max.is_some_and(|max| count < max) {
        let mut next = BTreeSet::new();
        for end in &current {
            if *end < len {
                next.extend(one(end + 1));
            }
        }
        count += 1;
        if count >= min {
            out.extend(next.iter().copied());
        }
        current = next;
        if current.is_empty() {
            return out;
        }
    }
    if max.is_none() {
        let mut seen = current.clone();
        let mut queue: Vec<i64> = current.into_iter().collect();
        while let Some(end) = queue.pop() {
            if end + 1 > len {
                continue;
            }
            for next in one(end + 1) {
                if seen.insert(next) {
                    queue.push(next);
                    out.insert(next);
                }
            }
        }
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
            for end in matches(seq, &word, 0) {
                expected.push(format!("{index:02} {trace} {}", end + 1));
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

#[test]
fn interpreter_follows_the_annex_f_empty_match_rules() {
    // Hand-checked anchors for the interpreter itself (16.9.2.1): with
    // `a` true everywhere, `(a[*0] ##0 b)` never matches, `(a[*0] ##1 b)` is
    // `b`, `(b ##1 a[*0])` is `b ##0 1`, and `a[*0:1]` matches empty and at 0.
    let word = [(true, true), (true, false)];
    let empty_fuse = Cat(bx(Rep(b(Atom::A), 0, Some(0))), 0, Some(0), b(Atom::B));
    assert!(matches(&empty_fuse, &word, 0).is_empty());
    let empty_cat = Cat(bx(Rep(b(Atom::A), 0, Some(0))), 1, Some(1), b(Atom::B));
    assert_eq!(matches(&empty_cat, &word, 0), BTreeSet::from([0]));
    let trailing = Cat(b(Atom::B), 1, Some(1), bx(Rep(b(Atom::A), 0, Some(0))));
    assert_eq!(matches(&trailing, &word, 0), BTreeSet::from([0]));
    assert_eq!(
        matches(&Rep(b(Atom::A), 0, Some(1)), &word, 0),
        BTreeSet::from([-1, 0])
    );
    // `and` ends at the later endpoint; `intersect` needs equal endpoints.
    let and = And(b(Atom::A), bx(Rep(b(Atom::A), 1, Some(2))));
    assert_eq!(matches(&and, &word, 0), BTreeSet::from([0, 1]));
    let intersect = Intersect(b(Atom::A), bx(Rep(b(Atom::A), 2, Some(2))));
    assert!(matches(&intersect, &word, 0).is_empty());
    // `##[0:1] r` = `r or (1 ##1 r)` keeps the empty match of `r` like
    // `##0 r` (F.3.4.2.2): `##[0:1] b[*0:1]` ends at -1 (empty) and at 0
    // (`b` on letter 0, or `1` followed by the empty `b[*0]`).
    let lead = Lead(0, Some(1), bx(Rep(b(Atom::B), 0, Some(1))));
    assert_eq!(matches(&lead, &word, 0), BTreeSet::from([-1, 0]));
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
