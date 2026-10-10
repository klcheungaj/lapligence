//! SIM-038: ordinary properties, actions and expect (IEEE 1800-2009
//! §§16.12-16.13, 16.15, 16.18, Annex F). A01 compares the public CLI against
//! a test-side property interpreter written from the Annex F neutral
//! satisfaction rules (F.5.3.1), the derived operators (F.3.4.3) and the
//! nonvacuity rules of 16.15.8; the other oracles are derived by hand in the
//! fixture readme.
use super::sim_cli;
use std::collections::HashMap;

const SUITE: &str = "feature_completion/sim_038";

// ---------------------------------------------------------------------------
// Annex F property interpreter (test side only; shares nothing with the
// runtime property engine).
//
// Words are over 2^{a,b} plus the letters T (top) and ⊥ (bottom) of F.5: T
// satisfies every Boolean, ⊥ none. A word is a finite letter list, optionally
// followed by one letter repeated forever, so every suffix that starts in the
// repeated tail is the same word. Negation evaluates its operand on the dual
// word (T and ⊥ exchanged), as F.5.3.1 defines `not`.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Letter {
    V(bool, bool),
    Top,
    Bot,
}

impl Letter {
    fn dual(self) -> Self {
        match self {
            Letter::Top => Letter::Bot,
            Letter::Bot => Letter::Top,
            other => other,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Atom {
    A,
    B,
    NotA,
    NotB,
    AandB,
    True,
    False,
}

impl Atom {
    fn holds(self, letter: Letter) -> bool {
        let (a, b) = match letter {
            Letter::Top => return true,
            Letter::Bot => return false,
            Letter::V(a, b) => (a, b),
        };
        match self {
            Atom::A => a,
            Atom::B => b,
            Atom::NotA => !a,
            Atom::NotB => !b,
            Atom::AandB => a && b,
            Atom::True => true,
            Atom::False => false,
        }
    }

    fn text(self) -> &'static str {
        match self {
            Atom::A => "a",
            Atom::B => "b",
            Atom::NotA => "!a",
            Atom::NotB => "!b",
            Atom::AandB => "(a && b)",
            Atom::True => "1'b1",
            Atom::False => "1'b0",
        }
    }
}

#[derive(Clone, Copy)]
struct Word<'a> {
    letters: &'a [Letter],
    tail: Option<Letter>,
    dual: bool,
}

impl<'a> Word<'a> {
    /// Number of distinct suffix positions: every letter of a finite word, or
    /// the finite part plus the one repeated-tail suffix class.
    fn positions(&self) -> usize {
        self.letters.len() + usize::from(self.tail.is_some())
    }

    fn infinite(&self) -> bool {
        self.tail.is_some()
    }

    fn letter(&self, i: usize) -> Option<Letter> {
        let raw = self.letters.get(i).copied().or(self.tail)?;
        Some(if self.dual { raw.dual() } else { raw })
    }

    fn suffix(&self, i: usize) -> Word<'a> {
        Word {
            letters: &self.letters[i.min(self.letters.len())..],
            ..*self
        }
    }

    fn dualized(&self) -> Word<'a> {
        Word {
            dual: !self.dual,
            ..*self
        }
    }

    /// `w_{0,i-1}` followed by T forever (in this word's own view).
    fn prefix_then_top(&self, i: usize) -> Word<'a> {
        Word {
            letters: &self.letters[..i.min(self.letters.len())],
            tail: Some(if self.dual { Letter::Bot } else { Letter::Top }),
            dual: self.dual,
        }
    }
}

/// A sequence operand as the set of letter-by-letter conjunctions it can
/// match: alternative `k` matches `w_{0,|k|-1}` when each of its atoms holds
/// on the corresponding letter. Every sequence in the suite has this form
/// (`##1`/`##[m:n]` chains of Booleans); SIM-037 owns general sequences.
type Seq = &'static [&'static [Atom]];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strength {
    /// No `strong`/`weak` keyword: weak in `assert`, strong in `cover`
    /// (F.3.4.3.1). The suite's sequences are fixed-length, so the two
    /// strengths decide every attempt on the same tick; they differ only
    /// at the end of simulation, which the trace harness never reaches.
    Implicit,
    Strong,
    Weak,
}

enum Prop {
    Seq(Strength, &'static str, Seq),
    Not(Box<Prop>),
    And(Box<Prop>, Box<Prop>),
    Or(Box<Prop>, Box<Prop>),
    Implies(Box<Prop>, Box<Prop>),
    Iff(Box<Prop>, Box<Prop>),
    /// `r |-> p` (overlapped) or `r |=> p`.
    Imp(&'static str, Seq, bool, Box<Prop>),
    /// `r #-# p` (overlapped) or `r #=# p`.
    Followed(&'static str, Seq, bool, Box<Prop>),
    If(Atom, Box<Prop>, Option<Box<Prop>>),
    /// `case (a) 1'b1: p1; default: p2; endcase`.
    CaseA(Box<Prop>, Box<Prop>),
    Next(u32, bool, Box<Prop>),
    /// `always`/`s_always` with `[min:max]`; `None` max is `$`, and a bare
    /// `always` is `(0, None, false, false)` with `bare` set.
    Always(u32, Option<u32>, bool, bool, Box<Prop>),
    Eventually(u32, Option<u32>, bool, bool, Box<Prop>),
    /// `p until q` with (strong, with).
    Until(bool, bool, Box<Prop>, Box<Prop>),
    /// `accept_on`/`reject_on` with (accept, sync).
    Abort(bool, bool, Atom, Box<Prop>),
}

use Prop::*;

fn bx(p: Prop) -> Box<Prop> {
    Box::new(p)
}

/// Core property forms of F.5.3.1; every surface operator lowers to these
/// through the F.3.4.3 derivations in [`core`].
#[derive(Clone)]
enum Core {
    Strong(Vec<Vec<Atom>>),
    Weak(Vec<Vec<Atom>>),
    Not(Box<Core>),
    And(Box<Core>, Box<Core>),
    Or(Box<Core>, Box<Core>),
    Imp(Vec<Vec<Atom>>, Box<Core>),
    Next(Box<Core>),
    Until(Box<Core>, Box<Core>),
    Accept(Atom, Box<Core>),
}

fn cb(c: Core) -> Box<Core> {
    Box::new(c)
}

fn seq_vec(seq: Seq) -> Vec<Vec<Atom>> {
    seq.iter().map(|alt| alt.to_vec()).collect()
}

/// `r ##1 1` (F.3.4.3.3).
fn then_true(seq: Seq) -> Vec<Vec<Atom>> {
    seq.iter()
        .map(|alt| {
            let mut alt = alt.to_vec();
            alt.push(Atom::True);
            alt
        })
        .collect()
}

fn c_not(c: Core) -> Core {
    Core::Not(cb(c))
}

fn c_implies(p: Core, q: Core) -> Core {
    Core::Or(cb(c_not(p)), cb(q))
}

/// `nexttime[0] p` = `1 |-> p`; `nexttime[m] p` = `nexttime(nexttime[m-1] p)`.
fn c_next(n: u32, p: Core) -> Core {
    (0..n).fold(Core::Imp(vec![vec![Atom::True]], cb(p)), |acc, _| {
        Core::Next(cb(acc))
    })
}

/// `always p` = `p until 0` (F.3.4.3.8).
fn c_always(p: Core) -> Core {
    Core::Until(cb(p), cb(Core::Weak(vec![vec![Atom::False]])))
}

fn c_s_eventually(p: Core) -> Core {
    c_not(c_always(c_not(p)))
}

/// `always[m:n] p` = `nexttime[m] p and ... and nexttime[n] p`, and
/// `always[m:$] p` = `nexttime[m] always p` (F.3.4.3.9).
fn c_always_range(min: u32, max: Option<u32>, p: &Core) -> Core {
    match max {
        None => c_next(min, c_always(p.clone())),
        Some(max) => (min + 1..=max).fold(c_next(min, p.clone()), |acc, k| {
            Core::And(cb(acc), cb(c_next(k, p.clone())))
        }),
    }
}

/// `eventually[m:n] p` = `nexttime[m] p or ... or nexttime[n] p` (F.3.4.3.9).
fn c_eventually_range(min: u32, max: u32, p: &Core) -> Core {
    (min + 1..=max).fold(c_next(min, p.clone()), |acc, k| {
        Core::Or(cb(acc), cb(c_next(k, p.clone())))
    })
}

fn c_until(strong: bool, p: Core, q: Core) -> Core {
    let weak = Core::Until(cb(p), cb(q.clone()));
    if strong {
        Core::And(cb(weak), cb(c_s_eventually(q)))
    } else {
        weak
    }
}

fn core(p: &Prop) -> Core {
    match p {
        Seq(strength, _, seq) => match strength {
            Strength::Strong => Core::Strong(seq_vec(seq)),
            Strength::Weak | Strength::Implicit => Core::Weak(seq_vec(seq)),
        },
        Not(p) => c_not(core(p)),
        And(p, q) => Core::And(cb(core(p)), cb(core(q))),
        Or(p, q) => Core::Or(cb(core(p)), cb(core(q))),
        Implies(p, q) => c_implies(core(p), core(q)),
        Iff(p, q) => Core::And(
            cb(c_implies(core(p), core(q))),
            cb(c_implies(core(q), core(p))),
        ),
        Imp(_, seq, overlapped, p) => Core::Imp(
            if *overlapped {
                seq_vec(seq)
            } else {
                then_true(seq)
            },
            cb(core(p)),
        ),
        // `r #-# p` = `not (r |-> not p)` (F.3.4.3.6).
        Followed(_, seq, overlapped, p) => c_not(Core::Imp(
            if *overlapped {
                seq_vec(seq)
            } else {
                then_true(seq)
            },
            cb(c_not(core(p))),
        )),
        // `if (b) P` = `b |-> P`; with else `(b |-> P1) and (weak(b) or P2)`.
        If(cond, then, otherwise) => {
            let imp = Core::Imp(vec![vec![*cond]], cb(core(then)));
            match otherwise {
                None => imp,
                Some(otherwise) => Core::And(
                    cb(imp),
                    cb(Core::Or(
                        cb(Core::Weak(vec![vec![*cond]])),
                        cb(core(otherwise)),
                    )),
                ),
            }
        }
        // F.3.4.3.5: `if (a === 1'b1) P1 else Pd`.
        CaseA(then, default) => core(&If(
            Atom::A,
            bx(clone_prop(then)),
            Some(bx(clone_prop(default))),
        )),
        Next(n, strong, p) => {
            if *strong {
                // `s_nexttime[m] p` = `not nexttime[m] not p`.
                c_not(c_next(*n, c_not(core(p))))
            } else {
                c_next(*n, core(p))
            }
        }
        Always(min, max, strong, _, p) => {
            let p = core(p);
            if *strong {
                // `s_always[m:n] p` = `not eventually[m:n] not p`.
                let max = max.expect("s_always is bounded");
                c_not(c_eventually_range(*min, max, &c_not(p)))
            } else {
                c_always_range(*min, *max, &p)
            }
        }
        Eventually(min, max, strong, bare, p) => {
            let p = core(p);
            match (strong, max) {
                (true, _) if *bare => c_s_eventually(p),
                // `s_eventually[m:$] p` = `s_nexttime[m] s_eventually p`.
                (true, None) => c_not(c_next(*min, c_not(c_s_eventually(p)))),
                // `s_eventually[m:n] p` = `not always[m:n] not p`.
                (true, Some(max)) => c_not(c_always_range(*min, Some(*max), &c_not(p))),
                (false, Some(max)) => c_eventually_range(*min, *max, &p),
                (false, None) => panic!("weak eventually is bounded"),
            }
        }
        Until(strong, with, p, q) => {
            let (p, q) = (core(p), core(q));
            let q = if *with {
                Core::And(cb(p.clone()), cb(q))
            } else {
                q
            };
            c_until(*strong, p, q)
        }
        // `reject_on(b) P` = `not accept_on(b) not P`; with clock context 1
        // the sync forms equal the async ones (F.3.4.3.7). The harness
        // changes signals only between clock ticks, so the sampled value
        // seen by the asynchronous check equals the tick's letter.
        Abort(accept, _, cond, p) => {
            if *accept {
                Core::Accept(*cond, cb(core(p)))
            } else {
                c_not(Core::Accept(*cond, cb(c_not(core(p)))))
            }
        }
    }
}

fn clone_prop(p: &Prop) -> Prop {
    match p {
        Seq(s, t, q) => Seq(*s, t, q),
        Not(p) => Not(bx(clone_prop(p))),
        And(p, q) => And(bx(clone_prop(p)), bx(clone_prop(q))),
        Or(p, q) => Or(bx(clone_prop(p)), bx(clone_prop(q))),
        Implies(p, q) => Implies(bx(clone_prop(p)), bx(clone_prop(q))),
        Iff(p, q) => Iff(bx(clone_prop(p)), bx(clone_prop(q))),
        Imp(t, s, o, p) => Imp(t, s, *o, bx(clone_prop(p))),
        Followed(t, s, o, p) => Followed(t, s, *o, bx(clone_prop(p))),
        If(c, p, q) => If(*c, bx(clone_prop(p)), q.as_ref().map(|q| bx(clone_prop(q)))),
        CaseA(p, q) => CaseA(bx(clone_prop(p)), bx(clone_prop(q))),
        Next(n, s, p) => Next(*n, *s, bx(clone_prop(p))),
        Always(m, n, s, b, p) => Always(*m, *n, *s, *b, bx(clone_prop(p))),
        Eventually(m, n, s, b, p) => Eventually(*m, *n, *s, *b, bx(clone_prop(p))),
        Until(s, w, p, q) => Until(*s, *w, bx(clone_prop(p)), bx(clone_prop(q))),
        Abort(a, s, c, p) => Abort(*a, *s, *c, bx(clone_prop(p))),
    }
}

/// `w_{0,|alt|-1}` tightly satisfies `alt` (F.5.2); the letters must exist.
fn alt_matches(alt: &[Atom], w: &Word) -> bool {
    alt.iter()
        .enumerate()
        .all(|(o, atom)| w.letter(o).is_some_and(|letter| atom.holds(letter)))
}

/// Neutral satisfaction `w |= p` (F.5.3.1).
fn sat(p: &Core, w: &Word) -> bool {
    match p {
        Core::Strong(alts) => alts.iter().any(|alt| alt_matches(alt, w)),
        Core::Weak(alts) => {
            // For every j < |w|, `w_{0,j} T^ω |= strong(R)`: some alternative
            // holds on every letter up to j (later letters are T). Beyond the
            // longest alternative the condition no longer changes.
            let longest = alts.iter().map(|alt| alt.len()).max().unwrap_or(0);
            let rows = if w.infinite() {
                longest.max(1)
            } else {
                w.positions()
            };
            (0..rows).all(|j| {
                alts.iter().any(|alt| {
                    alt.iter()
                        .take(j + 1)
                        .enumerate()
                        .all(|(o, atom)| w.letter(o).is_some_and(|letter| atom.holds(letter)))
                })
            })
        }
        Core::Not(p) => !sat(p, &w.dualized()),
        Core::And(p, q) => sat(p, w) && sat(q, w),
        Core::Or(p, q) => sat(p, w) || sat(q, w),
        Core::Imp(alts, p) => alts
            .iter()
            .filter(|alt| alt_matches(alt, w))
            .all(|alt| sat(p, &w.suffix(alt.len() - 1))),
        // `nexttime P` iff `|w| = 0` or `w^{1..} |= P`.
        Core::Next(p) => w.positions() == 0 || sat(p, &w.suffix(1)),
        Core::Until(p, q) => {
            let n = w.positions();
            (0..n).any(|j| sat(q, &w.suffix(j)) && (0..j).all(|i| sat(p, &w.suffix(i))))
                || (0..n).all(|i| sat(p, &w.suffix(i)))
        }
        Core::Accept(cond, p) => {
            sat(p, w)
                || (0..w.positions()).any(|i| {
                    w.letter(i).is_some_and(|letter| cond.holds(letter))
                        && sat(p, &w.prefix_then_top(i))
                })
        }
    }
}

/// Nonvacuity of a passing evaluation attempt on the finite word `w` of its
/// ticks (16.15.8; Annex F F.5.3.3 for the same rules). "Holds" is neutral
/// satisfaction on that word.
fn nonvacuous(p: &Prop, w: &Word) -> bool {
    let holds = |p: &Prop, i: usize| sat(&core(p), &w.suffix(i));
    let n = w.positions();
    let matches_at = |seq: Seq, overlapped: bool| -> Vec<usize> {
        seq.iter()
            .filter(|alt| alt_matches(alt, w))
            .map(|alt| alt.len() - 1 + usize::from(!overlapped))
            .filter(|start| *start < n)
            .collect()
    };
    match p {
        // a)-c)
        Seq(..) => true,
        // d)
        Not(p) => nonvacuous(p, w),
        // e), f), aa)
        And(p, q) | Or(p, q) | Iff(p, q) => nonvacuous(p, w) || nonvacuous(q, w),
        // z)
        Implies(p, _) => nonvacuous(p, w),
        // h), j), k)
        Imp(_, seq, overlapped, p) | Followed(_, seq, overlapped, p) => {
            matches_at(seq, *overlapped)
                .into_iter()
                .any(|start| nonvacuous(p, &w.suffix(start)))
        }
        // g), ad)
        If(cond, then, otherwise) => {
            let taken = w.letter(0).is_some_and(|letter| cond.holds(letter));
            if taken {
                nonvacuous(then, w)
            } else {
                otherwise.as_ref().is_some_and(|p| nonvacuous(p, w))
            }
        }
        CaseA(then, default) => {
            if w.letter(0).is_some_and(|letter| Atom::A.holds(letter)) {
                nonvacuous(then, w)
            } else {
                nonvacuous(default, w)
            }
        }
        // l)-o)
        Next(k, _, p) => (*k as usize) < n && nonvacuous(p, &w.suffix(*k as usize)),
        // p)-r): some event in range is nonvacuous and p holds before it.
        Always(min, max, _, _, p) => {
            let top = max.map_or(n, |max| (max as usize + 1).min(n));
            (*min as usize..top)
                .any(|i| nonvacuous(p, &w.suffix(i)) && (*min as usize..i).all(|j| holds(p, j)))
        }
        // s)-u): some event in range is nonvacuous and p does not hold before.
        Eventually(min, max, _, _, p) => {
            let top = max.map_or(n, |max| (max as usize + 1).min(n));
            (*min as usize..top)
                .any(|i| nonvacuous(p, &w.suffix(i)) && (*min as usize..i).all(|j| !holds(p, j)))
        }
        // v)-y): until counts both operands, until_with only the left one.
        Until(_, with, p, q) => (0..n).any(|i| {
            (nonvacuous(p, &w.suffix(i)) || (!with && nonvacuous(q, &w.suffix(i))))
                && (0..i).all(|j| holds(p, j) && !holds(q, j))
        }),
        // ab), ac)
        Abort(_, _, cond, p) => {
            nonvacuous(p, w)
                && (0..n).all(|i| !w.letter(i).is_some_and(|letter| cond.holds(letter)))
        }
    }
}

fn seq_text(strength: Strength, text: &str) -> String {
    let keyword = match strength {
        Strength::Implicit => return text.to_owned(),
        Strength::Strong => "strong",
        Strength::Weak => "weak",
    };
    if text.starts_with('(') {
        format!("{keyword}{text}")
    } else {
        format!("{keyword}({text})")
    }
}

fn range_text(min: u32, max: Option<u32>) -> String {
    match max {
        Some(max) => format!("[{min}:{max}]"),
        None => format!("[{min}:$]"),
    }
}

/// SystemVerilog text of `p`; the fixture must contain exactly this text.
fn render(p: &Prop) -> String {
    match p {
        Seq(strength, text, _) => seq_text(*strength, text),
        Not(p) => format!("(not {})", render(p)),
        And(p, q) => format!("({} and {})", render(p), render(q)),
        Or(p, q) => format!("({} or {})", render(p), render(q)),
        Implies(p, q) => format!("({} implies {})", render(p), render(q)),
        Iff(p, q) => format!("({} iff {})", render(p), render(q)),
        Imp(text, _, overlapped, p) => {
            let op = if *overlapped { "|->" } else { "|=>" };
            format!("({text} {op} {})", render(p))
        }
        Followed(text, _, overlapped, p) => {
            let op = if *overlapped { "#-#" } else { "#=#" };
            format!("({text} {op} {})", render(p))
        }
        If(cond, then, None) => format!("(if ({}) {})", cond.text(), render(then)),
        If(cond, then, Some(otherwise)) => format!(
            "(if ({}) {} else {})",
            cond.text(),
            render(then),
            render(otherwise)
        ),
        CaseA(then, default) => format!(
            "(case (a) 1'b1: {}; default: {}; endcase)",
            render(then),
            render(default)
        ),
        Next(n, strong, p) => {
            let op = if *strong { "s_nexttime" } else { "nexttime" };
            if *n == 1 {
                format!("({op} {})", render(p))
            } else {
                format!("({op} [{n}] {})", render(p))
            }
        }
        Always(min, max, strong, bare, p) => {
            let op = if *strong { "s_always" } else { "always" };
            if *bare {
                format!("({op} {})", render(p))
            } else {
                format!("({op} {} {})", range_text(*min, *max), render(p))
            }
        }
        Eventually(min, max, strong, bare, p) => {
            let op = if *strong {
                "s_eventually"
            } else {
                "eventually"
            };
            if *bare {
                format!("({op} {})", render(p))
            } else {
                format!("({op} {} {})", range_text(*min, *max), render(p))
            }
        }
        Until(strong, with, p, q) => {
            let op = match (strong, with) {
                (false, false) => "until",
                (true, false) => "s_until",
                (false, true) => "until_with",
                (true, true) => "s_until_with",
            };
            format!("({} {op} {})", render(p), render(q))
        }
        Abort(accept, sync, cond, p) => {
            let op = match (accept, sync) {
                (true, false) => "accept_on",
                (false, false) => "reject_on",
                (true, true) => "sync_accept_on",
                (false, true) => "sync_reject_on",
            };
            format!("({op} ({}) {})", cond.text(), render(p))
        }
    }
}

const SA: Seq = &[&[Atom::A]];
const SB: Seq = &[&[Atom::B]];
const SNOTB: Seq = &[&[Atom::NotB]];
const SNOTA: Seq = &[&[Atom::NotA]];
const SAB: Seq = &[&[Atom::A, Atom::B]];
const SAANDB: Seq = &[&[Atom::AandB]];

fn a() -> Box<Prop> {
    bx(Seq(Strength::Implicit, "a", SA))
}

fn b() -> Box<Prop> {
    bx(Seq(Strength::Implicit, "b", SB))
}

fn not_a() -> Box<Prop> {
    bx(Seq(Strength::Implicit, "!a", SNOTA))
}

fn not_b() -> Box<Prop> {
    bx(Seq(Strength::Implicit, "!b", SNOTB))
}

fn a_and_b() -> Box<Prop> {
    bx(Seq(Strength::Implicit, "(a && b)", SAANDB))
}

fn next(p: Box<Prop>) -> Box<Prop> {
    bx(Next(1, false, p))
}

fn always_range(min: u32, max: Option<u32>, p: Box<Prop>) -> Box<Prop> {
    bx(Always(min, max, false, false, p))
}

/// The properties of `exhaustive.sv`, in label order.
fn exhaustive_properties() -> Vec<Prop> {
    use Atom::{AandB, NotA, True, A, B};
    vec![
        Seq(Strength::Implicit, "a", SA),
        Seq(Strength::Strong, "(a ##1 b)", SAB),
        Seq(Strength::Weak, "(a ##[1:2] b)", &[&[A, B], &[A, True, B]]),
        Not(bx(Seq(Strength::Implicit, "(a ##1 b)", SAB))),
        Not(bx(Seq(
            Strength::Strong,
            "(a ##[0:1] b)",
            &[&[AandB], &[A, B]],
        ))),
        Or(bx(Seq(Strength::Implicit, "(a ##1 b)", SAB)), next(a())),
        And(
            bx(Imp("a", SA, true, next(b()))),
            bx(Imp("b", SB, true, next(a()))),
        ),
        If(A, next(b()), None),
        If(A, next(b()), Some(always_range(0, Some(1), not_b()))),
        CaseA(next(b()), always_range(0, Some(1), not_b())),
        Imp("a", SA, true, bx(Until(false, false, b(), a()))),
        Followed("(a ##1 b)", SAB, true, next(a())),
        Followed("a", SA, false, bx(Until(false, true, b(), a()))),
        Implies(
            next(a()),
            bx(Seq(Strength::Implicit, "(b ##1 b)", &[&[B, B]])),
        ),
        Iff(next(a()), b()),
        Next(2, false, a()),
        Next(1, true, a()),
        Next(2, true, a_and_b()),
        Next(0, false, b()),
        Always(1, Some(3), false, false, a()),
        Always(0, Some(2), true, false, b()),
        Always(0, None, false, true, a()),
        Always(2, None, false, false, a()),
        Eventually(0, None, true, true, a()),
        Eventually(1, Some(2), false, false, b()),
        Eventually(1, Some(3), true, false, a_and_b()),
        Eventually(2, None, true, false, b()),
        Until(false, false, a(), b()),
        Until(true, false, a(), b()),
        Until(false, true, a(), b()),
        Until(true, true, a(), b()),
        Abort(true, false, B, bx(Always(0, None, false, true, a()))),
        Abort(
            false,
            false,
            B,
            bx(Seq(
                Strength::Implicit,
                "(a ##[1:3] a)",
                &[&[A, A], &[A, True, A], &[A, True, True, A]],
            )),
        ),
        Abort(
            true,
            true,
            AandB,
            bx(Imp("b", SB, false, always_range(0, Some(2), not_a()))),
        ),
        Abort(
            false,
            true,
            NotA,
            bx(Seq(Strength::Implicit, "(a ##2 b)", &[&[A, True, B]])),
        ),
        Always(0, Some(2), false, false, bx(Imp("a", SA, true, next(b())))),
        Eventually(0, Some(2), true, false, bx(And(a(), next(a())))),
        Not(bx(Until(false, false, a(), b()))),
        Or(
            always_range(0, Some(1), a()),
            bx(Eventually(0, Some(2), true, false, b())),
        ),
        Imp("a", SA, false, bx(Implies(b(), bx(Next(1, true, a()))))),
        Abort(
            true,
            false,
            AandB,
            bx(Imp("b", SB, false, always_range(0, Some(2), not_a()))),
        ),
        Not(bx(Eventually(1, Some(2), true, false, a()))),
        Imp("(a ##[0:1] b)", &[&[AandB], &[A, B]], true, next(a())),
        Always(0, Some(3), false, false, bx(Or(b(), next(b())))),
        Until(
            true,
            false,
            bx(Seq(Strength::Implicit, "(a ##1 a)", &[&[A, A]])),
            b(),
        ),
    ]
}

const TRACE_LENGTH: usize = 5;

const LETTERS: [Letter; 4] = [
    Letter::V(false, false),
    Letter::V(true, false),
    Letter::V(false, true),
    Letter::V(true, true),
];

/// Whether the finite prefix `u` already decides `p`: `Some(result)` when
/// every continuation `u v c^ω` (`|v| <= 2`, `v` and `c` real letters) gives
/// the same result. An attempt is reported on the first tick whose prefix
/// decides it; one that no prefix of the trace decides stays pending.
fn decide(p: &Core, u: &[Letter]) -> Option<bool> {
    let mut seen = [false, false];
    let mut word = u.to_vec();
    let mut check = |word: &[Letter]| {
        for tail in LETTERS {
            let w = Word {
                letters: word,
                tail: Some(tail),
                dual: false,
            };
            seen[usize::from(sat(p, &w))] = true;
        }
    };
    check(&word);
    for x in LETTERS {
        word.push(x);
        check(&word);
        for y in LETTERS {
            word.push(y);
            check(&word);
            word.pop();
        }
        word.pop();
    }
    match seen {
        [true, true] => None,
        [false, true] => Some(true),
        [true, false] => Some(false),
        [false, false] => unreachable!(),
    }
}

fn trace_word(trace: u32) -> Vec<Letter> {
    (0..TRACE_LENGTH)
        .map(|k| Letter::V((trace >> (2 * k)) & 1 == 1, (trace >> (2 * k + 1)) & 1 == 1))
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Pass { tick: usize, nonvacuous: bool },
    Fail { tick: usize },
    Pending,
}

/// The outcome of the attempt of `p` that starts on the first letter of
/// `word`; ticks count letters from 1.
fn outcome(
    p: &Prop,
    core: &Core,
    word: &[Letter],
    cache: &mut HashMap<Vec<Letter>, Option<bool>>,
) -> Outcome {
    for k in 0..word.len() {
        let prefix = &word[..=k];
        let decision = match cache.get(prefix) {
            Some(decision) => *decision,
            None => {
                let decision = decide(core, prefix);
                cache.insert(prefix.to_vec(), decision);
                decision
            }
        };
        match decision {
            Some(true) => {
                let w = Word {
                    letters: &word[..=k],
                    tail: None,
                    dual: false,
                };
                return Outcome::Pass {
                    tick: k + 1,
                    nonvacuous: nonvacuous(p, &w),
                };
            }
            Some(false) => return Outcome::Fail { tick: k + 1 },
            None => {}
        }
    }
    Outcome::Pending
}

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(name),
    )
    .expect("sim_038 fixture")
}

#[test]
fn properties_match_the_annex_f_interpreter_on_every_trace() {
    let properties = exhaustive_properties();
    let source = fixture_text("exhaustive.sv");
    let mut expected = Vec::new();
    for (index, p) in properties.iter().enumerate() {
        let text = render(p);
        let assert_line = format!(
            "  p{index:02}: assert property (@(posedge clk) disable iff (kill) go |=> {text}) $display(\"A{index:02} %0d %0d\", trace, pos); else $display(\"F{index:02} %0d %0d\", trace, pos);"
        );
        let cover_line = format!(
            "  c{index:02}: cover property (@(posedge clk) disable iff (kill) go |=> {text}) $display(\"C{index:02} %0d %0d\", trace, pos);"
        );
        for line in [&assert_line, &cover_line] {
            assert!(
                source.contains(line.as_str()),
                "fixture lacks interpreter property {index}: {line}"
            );
        }
        let core = core(p);
        let mut cache = HashMap::new();
        for trace in 0..(1u32 << (2 * TRACE_LENGTH)) {
            // Ticks 1..=5 also start attempts with `go` low: each succeeds
            // vacuously at once, so the assert prints and the cover does not.
            for pos in 1..=TRACE_LENGTH {
                expected.push(format!("A{index:02} {trace} {pos}"));
            }
            match outcome(p, &core, &trace_word(trace), &mut cache) {
                Outcome::Pass { tick, nonvacuous } => {
                    expected.push(format!("A{index:02} {trace} {tick}"));
                    if nonvacuous {
                        expected.push(format!("C{index:02} {trace} {tick}"));
                    }
                }
                Outcome::Fail { tick } => expected.push(format!("F{index:02} {trace} {tick}")),
                Outcome::Pending => {}
            }
        }
    }
    assert_eq!(
        source.matches(": assert property").count(),
        properties.len(),
        "fixture and interpreter list different properties"
    );
    expected.sort();
    sim_cli::run_case_checked_matrix(SUITE, "exhaustive", &[], &|label, output| {
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual = sorted_lines(&String::from_utf8_lossy(&output.stdout));
        if actual != expected {
            let missing: Vec<_> = expected
                .iter()
                .filter(|line| actual.binary_search(line).is_err())
                .take(8)
                .collect();
            let extra: Vec<_> = actual
                .iter()
                .filter(|line| expected.binary_search(line).is_err())
                .take(8)
                .collect();
            panic!(
                "{label}: {} lines, interpreter has {}; missing {missing:?}; unexpected {extra:?}",
                actual.len(),
                expected.len()
            );
        }
    });
}

fn assert_sorted_stdout(label: &str, output: &std::process::Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        sorted_lines(&String::from_utf8_lossy(&output.stdout)),
        sorted_lines(expected),
        "{label}: action lines differ from the hand-derived set"
    );
}

#[test]
fn interpreter_follows_the_annex_f_anchors() {
    let letter = |a: bool, b: bool| Letter::V(a, b);
    // Weak `nexttime a` waits for the next tick; on it the result is fixed.
    let next_a = core(&Next(1, false, a()));
    assert_eq!(decide(&next_a, &[letter(false, false)]), None);
    assert_eq!(
        decide(&next_a, &[letter(false, false), letter(true, false)]),
        Some(true)
    );
    // `s_eventually a` never fails on a finite prefix; `always a` never passes.
    let eventually_a = core(&Eventually(0, None, true, true, a()));
    assert_eq!(decide(&eventually_a, &[letter(false, true); 3]), None);
    let always_a = core(&Always(0, None, false, true, a()));
    assert_eq!(decide(&always_a, &[letter(true, false); 3]), None);
    assert_eq!(
        decide(&always_a, &[letter(true, false), letter(false, false)]),
        Some(false)
    );
    // `accept_on (b)` succeeds on the first tick where b holds, vacuously
    // (16.15.8 ab), even though its operand would fail there.
    let p = Abort(true, false, Atom::B, bx(Always(0, None, false, true, a())));
    let word = [letter(true, false), letter(false, true)];
    assert_eq!(decide(&core(&p), &word[..1]), None);
    assert_eq!(decide(&core(&p), &word), Some(true));
    let w = Word {
        letters: &word,
        tail: None,
        dual: false,
    };
    assert!(!nonvacuous(&p, &w));
    // `if (a) nexttime b` with a false holds at once and vacuously (16.15.8 g).
    let p = If(Atom::A, next(b()), None);
    let word = [letter(false, false)];
    assert_eq!(decide(&core(&p), &word), Some(true));
    let w = Word {
        letters: &word,
        tail: None,
        dual: false,
    };
    assert!(!nonvacuous(&p, &w));
}

#[test]
fn outcomes_distinguish_no_else_failures_disables_aborts_and_end_of_simulation() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_038/outcomes.out");
    sim_cli::run_case_checked_matrix(SUITE, "outcomes", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let failures: Vec<&str> = stderr
            .lines()
            .filter(|line| line.starts_with("llg: assertion assert failed: "))
            .collect();
        assert_eq!(failures.len(), 2, "{label}: {stderr}");
        assert!(
            failures[0].ends_with("outcomes.sv:22:3 (n1)")
                && failures[1].ends_with("outcomes.sv:48:3 (ens)"),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn overlapping_actions_suspend_independently_and_expect_resumes_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_038/actions.out");
    sim_cli::run_case_checked_matrix(SUITE, "actions", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected)
    });
}

#[test]
fn disable_conditions_and_default_disable_resolution() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_038/disable.out");
    sim_cli::run_case_checked_matrix(SUITE, "disable", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected)
    });
}

#[test]
fn initial_procedure_assertion_starts_one_attempt() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_038/initial_assert.out");
    sim_cli::run_case_checked_matrix(SUITE, "initial_assert", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected)
    });
}

// Illegal property grammar stays a frontend error; legal forms outside the
// SIM-038 subset keep specific located diagnostics.

#[test]
fn neg_s_always_unbounded() {
    sim_cli::reject_case(
        SUITE,
        "neg_s_always_unbounded",
        "unbounded literal '$' not allowed here",
    );
}

#[test]
fn neg_weak_eventually_unbounded() {
    sim_cli::reject_case(
        SUITE,
        "neg_weak_eventually_unbounded",
        "unbounded literal '$' not allowed here",
    );
}

#[test]
fn neg_nested_disable() {
    sim_cli::reject_case(SUITE, "neg_nested_disable", "expected expression");
}

#[test]
fn neg_empty_match_property() {
    sim_cli::reject_case(
        SUITE,
        "neg_empty_match_property",
        "sequence must not admit an empty match",
    );
}

#[test]
fn neg_multiclock_property() {
    sim_cli::reject_case(
        SUITE,
        "neg_multiclock_property",
        "multiclock properties are not supported: a clocking event inside the property differs from its leading clock",
    );
}

#[test]
fn neg_conflicting_clock() {
    sim_cli::reject_case(
        SUITE,
        "neg_conflicting_clock",
        "multiclock properties are not supported: a clocking event inside the property differs from its leading clock",
    );
}

#[test]
fn neg_recursive_property() {
    sim_cli::reject_case(
        SUITE,
        "neg_recursive_property",
        "recursive property instances are not supported",
    );
}
