//! Independent Annex F interpreter for the ADV-013 A01 oracle.
//!
//! Clocked properties are rewritten to unclocked ones by the clock rewrite
//! rules of F.5.1 (`T_s`, `T_p`) after expanding the derived forms of
//! F.3.4.3, recursive instances follow the k-fold approximations of F.7, and
//! unclocked properties are evaluated by the neutral satisfaction rules of
//! F.5.2/F.5.3.1. Nothing here is shared with the simulator's property
//! engine.
//!
//! A word has one letter per time step in which a clock of the test ticks:
//! the sampled values of `a` and `b` and which of `posedge c1` and
//! `posedge c2` occur. Time steps without a tick are left out: the tested
//! properties have no asynchronous abort, so they cannot observe them, and
//! `@(1)` (the `|=>` rewrite of F.3.4.3.3) then moves to the next ticking
//! step, after which every operand waits for its own clock anyway. The
//! letters T and ⊥ of F.5 satisfy every Boolean and no Boolean.

use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Letter {
    V {
        a: bool,
        b: bool,
        c1: bool,
        c2: bool,
    },
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

/// A Boolean over one letter.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) enum Cond {
    A,
    B,
    C1,
    C2,
    One,
    Not(Box<Cond>),
    And(Box<Cond>, Box<Cond>),
}

impl Cond {
    /// `ζ |= b` (F.5): T satisfies every Boolean and ⊥ none, so the letter
    /// kind is decided before the expression is looked at.
    fn holds(&self, letter: Letter) -> bool {
        match letter {
            Letter::Top => true,
            Letter::Bot => false,
            Letter::V { a, b, c1, c2 } => self.value(a, b, c1, c2),
        }
    }

    fn value(&self, a: bool, b: bool, c1: bool, c2: bool) -> bool {
        match self {
            Cond::A => a,
            Cond::B => b,
            Cond::C1 => c1,
            Cond::C2 => c2,
            Cond::One => true,
            Cond::Not(x) => !x.value(a, b, c1, c2),
            Cond::And(x, y) => x.value(a, b, c1, c2) && y.value(a, b, c1, c2),
        }
    }
}

fn not(b: Cond) -> Cond {
    Cond::Not(Box::new(b))
}

fn and(x: Cond, y: Cond) -> Cond {
    Cond::And(Box::new(x), Box::new(y))
}

/// Clocked sequences as written in the fixture.
#[derive(Clone, Debug)]
pub(super) enum S {
    Bool(Cond),
    /// `##1`
    Cat(Box<S>, Box<S>),
    /// `##0`
    Fuse(Box<S>, Box<S>),
    Clock(Cond, Box<S>),
}

/// Clocked properties as written in the fixture, with the derived forms
/// expanded by [`lower`].
#[derive(Clone, Debug)]
pub(super) enum Q {
    Weak(S),
    Strong(S),
    Clock(Cond, Box<Q>),
    Not(Box<Q>),
    And(Box<Q>, Box<Q>),
    Or(Box<Q>, Box<Q>),
    /// `|->`
    Imp(S, Box<Q>),
    /// `|=>`
    NonImp(S, Box<Q>),
    IfElse(Cond, Box<Q>, Box<Q>),
    Next(Box<Q>),
    Until(Box<Q>, Box<Q>),
    Always(Box<Q>),
    SyncAccept(Cond, Box<Q>),
    /// An instance of named property `n` of the [`Program`].
    Inst(usize),
}

/// Unclocked sequences (F.3.2).
#[derive(Clone, Debug)]
enum R {
    Bool(Cond),
    Cat(Box<R>, Box<R>),
    Fuse(Box<R>, Box<R>),
    Or(Box<R>, Box<R>),
    /// `R[*0]` of anything: the empty word.
    Empty,
    /// `R[*1:$]`
    Plus(Box<R>),
}

/// Unclocked properties (F.3.2).
#[derive(Clone, Debug)]
enum P {
    /// `strong(R)`, with the clock of the first tick of the clocked
    /// sequence it was rewritten from (see [`Interp::sat`]).
    Strong(R, Cond),
    Weak(R),
    Not(Box<P>),
    And(Box<P>, Box<P>),
    Or(Box<P>, Box<P>),
    Imp(R, Box<P>),
    Next(Box<P>),
    Until(Box<P>, Box<P>),
    Accept(Cond, Box<P>),
    /// A named-property instance rewritten under one clock: index into the
    /// interpreter's call table.
    Call(usize),
}

fn bx<T>(x: T) -> Box<T> {
    Box::new(x)
}

/// `T_s(S, c)` (F.5.1.1).
fn t_s(s: &S, c: &Cond) -> R {
    match s {
        // T_s(b, c) = (!c[*0:$] ##1 c & b), with R[*0:$] = R[*0] or R[*1:$].
        S::Bool(b) => R::Cat(
            bx(R::Or(
                bx(R::Empty),
                bx(R::Plus(bx(R::Bool(not(c.clone()))))),
            )),
            bx(R::Bool(and(c.clone(), b.clone()))),
        ),
        S::Cat(x, y) => R::Cat(bx(t_s(x, c)), bx(t_s(y, c))),
        S::Fuse(x, y) => R::Fuse(bx(t_s(x, c)), bx(t_s(y, c))),
        S::Clock(c2, x) => t_s(x, c2),
    }
}

/// The clock of the first tick of a clocked sequence under clock `c`.
fn leading(s: &S, c: &Cond) -> Cond {
    match s {
        S::Bool(_) => c.clone(),
        S::Cat(x, _) | S::Fuse(x, _) => leading(x, c),
        S::Clock(c2, x) => leading(x, c2),
    }
}

/// The named properties of one test: bodies indexed by [`Q::Inst`].
pub(super) struct Program {
    pub(super) bodies: Vec<Q>,
}

/// The rewritten instance bodies, one per (named property, clock).
struct Calls<'p> {
    program: &'p Program,
    keys: Vec<(usize, Cond)>,
    bodies: Vec<P>,
    /// `T_p(1'b1, c)`: the body of the 0-fold approximation (F.7).
    zero: Vec<P>,
}

impl Calls<'_> {
    /// `T_p(Q, c)` (F.5.1.2), after the derived forms of F.3.4.3.
    fn t_p(&mut self, q: &Q, c: &Cond) -> P {
        match q {
            Q::Weak(s) => P::Weak(t_s(s, c)),
            Q::Strong(s) => P::Strong(t_s(s, c), leading(s, c)),
            Q::Clock(c2, q) => self.t_p(q, c2),
            Q::Not(q) => P::Not(bx(self.t_p(q, c))),
            Q::And(x, y) => P::And(bx(self.t_p(x, c)), bx(self.t_p(y, c))),
            Q::Or(x, y) => P::Or(bx(self.t_p(x, c)), bx(self.t_p(y, c))),
            Q::Imp(s, q) => P::Imp(t_s(s, c), bx(self.t_p(q, c))),
            // F.3.4.3.3: (S |=> Q) is ((S ##1 @(1) 1) |-> Q).
            Q::NonImp(s, q) => {
                let s = S::Cat(
                    bx(s.clone()),
                    bx(S::Clock(Cond::One, bx(S::Bool(Cond::One)))),
                );
                self.t_p(&Q::Imp(s, q.clone()), c)
            }
            // F.3.4.3.4: (if(b) P1 else P2) is ((b |-> P1) and (weak(b) or P2)).
            Q::IfElse(b, x, y) => {
                let q = Q::And(
                    bx(Q::Imp(S::Bool(b.clone()), x.clone())),
                    bx(Q::Or(bx(Q::Weak(S::Bool(b.clone()))), y.clone())),
                );
                self.t_p(&q, c)
            }
            // T_p(nexttime p, c) = (!c until (c and nexttime (!c until (c and T_p(p, c))))).
            Q::Next(q) => {
                let inner = self.t_p(q, c);
                let wait = |p: P| {
                    P::Until(
                        bx(P::Weak(R::Bool(not(c.clone())))),
                        bx(P::And(bx(P::Weak(R::Bool(c.clone()))), bx(p))),
                    )
                };
                wait(P::Next(bx(wait(inner))))
            }
            // T_p(p1 until p2, c) = ((not (c and not T_p(p1, c))) until (c and T_p(p2, c))).
            Q::Until(x, y) => {
                let x = self.t_p(x, c);
                let y = self.t_p(y, c);
                P::Until(
                    bx(P::Not(bx(P::And(
                        bx(P::Weak(R::Bool(c.clone()))),
                        bx(P::Not(bx(x))),
                    )))),
                    bx(P::And(bx(P::Weak(R::Bool(c.clone()))), bx(y))),
                )
            }
            // F.3.4.3.8: (always p) is (p until 0).
            Q::Always(q) => {
                let zero = Q::Weak(S::Bool(not(Cond::One)));
                self.t_p(&Q::Until(q.clone(), bx(zero)), c)
            }
            // T_p(sync_accept_on(b) p, c) = (accept_on(b && c) T_p(p, c)).
            Q::SyncAccept(b, q) => P::Accept(and(b.clone(), c.clone()), bx(self.t_p(q, c))),
            Q::Inst(name) => P::Call(self.call(*name, c)),
        }
    }

    /// The call-table entry of named property `name` under clock `c`; the
    /// entry exists before its body is rewritten, so recursive instances
    /// refer back to it.
    fn call(&mut self, name: usize, c: &Cond) -> usize {
        if let Some(index) = self
            .keys
            .iter()
            .position(|(known, clock)| *known == name && clock == c)
        {
            return index;
        }
        let index = self.keys.len();
        self.keys.push((name, c.clone()));
        self.bodies.push(P::Strong(R::Empty, Cond::One));
        self.zero.push(P::Weak(t_s(&S::Bool(Cond::One), c)));
        let body = self.program.bodies[name].clone();
        self.bodies[index] = self.t_p(&body, c);
        index
    }
}

/// A word over Σ: finite letters, then either nothing (a finite word) or one
/// letter repeated forever. Position `letters.len()` stands for every
/// suffix inside the repeated tail, which are all the same word.
#[derive(Clone, Copy)]
struct Word<'a> {
    letters: &'a [Letter],
    tail: Option<Letter>,
    dual: bool,
}

impl Word<'_> {
    fn len(&self) -> usize {
        self.letters.len()
    }

    fn letter(&self, i: usize) -> Option<Letter> {
        let raw = self.letters.get(i).copied().or(self.tail)?;
        Some(if self.dual { raw.dual() } else { raw })
    }

    /// The suffix position after the letter at `i`.
    fn after(&self, i: usize) -> usize {
        (i + 1).min(self.len())
    }

    /// Positions that start a nonempty suffix.
    fn starts(&self) -> std::ops::Range<usize> {
        0..self.len() + usize::from(self.tail.is_some())
    }

    fn dualized(&self) -> Self {
        Word {
            dual: !self.dual,
            ..*self
        }
    }
}

/// End points of the tight matches of a sequence from one position: whether
/// it matches the empty word there, and the positions of the last letters of
/// its nonempty matches.
#[derive(Default)]
struct Ends {
    empty: bool,
    lasts: BTreeSet<usize>,
}

/// `w_{s..} ⊢ R` tight matches (F.5.2).
fn ends(r: &R, w: &Word, s: usize) -> Ends {
    match r {
        R::Bool(b) => Ends {
            empty: false,
            lasts: w
                .letter(s)
                .filter(|letter| b.holds(*letter))
                .map(|_| s)
                .into_iter()
                .collect(),
        },
        R::Empty => Ends {
            empty: true,
            lasts: BTreeSet::new(),
        },
        R::Cat(x, y) => {
            let first = ends(x, w, s);
            let mut out = Ends::default();
            if first.empty {
                let second = ends(y, w, s);
                out.empty = second.empty;
                out.lasts.extend(second.lasts);
            }
            for last in first.lasts {
                let second = ends(y, w, w.after(last));
                if second.empty {
                    out.lasts.insert(last);
                }
                out.lasts.extend(second.lasts);
            }
            out
        }
        R::Fuse(x, y) => {
            let first = ends(x, w, s);
            let mut out = Ends::default();
            for last in first.lasts {
                out.lasts.extend(ends(y, w, last).lasts);
            }
            out
        }
        R::Or(x, y) => {
            let mut out = ends(x, w, s);
            let other = ends(y, w, s);
            out.empty |= other.empty;
            out.lasts.extend(other.lasts);
            out
        }
        R::Plus(x) => {
            let first = ends(x, w, s);
            let mut out = Ends {
                empty: first.empty,
                lasts: BTreeSet::new(),
            };
            let mut todo: Vec<usize> = first.lasts.into_iter().collect();
            while let Some(last) = todo.pop() {
                if out.lasts.insert(last) {
                    todo.extend(ends(x, w, w.after(last)).lasts);
                }
            }
            out
        }
    }
}

/// Recursion depth used for the k-fold approximations of F.7. Every
/// recursive instance follows a positive advance in time (16.13.17
/// restriction 3), so beyond the finite letters each level only revisits the
/// uniform tail, where the approximations settle after one level: `len + 4`
/// levels decide every word the interpreter sees.
fn depth_for(w: &Word) -> usize {
    w.len() + 4
}

/// Extra tail letters examined by the prefix quantifiers of `weak` and
/// `accept_on`: a tail letter repeats, so a sequence's live states settle
/// after as many letters as the sequence has operators.
const TAIL_LOOKAHEAD: usize = 6;

/// Memo key of one instance evaluation: call, depth, position and word.
type MemoKey = (usize, usize, usize, Vec<Letter>, Option<Letter>, bool);

struct Interp<'p> {
    calls: Calls<'p>,
    /// Instance results of the current top-level evaluation; a property
    /// with two recursive call sites would otherwise take time exponential
    /// in the depth.
    memo: std::cell::RefCell<HashMap<MemoKey, bool>>,
}

impl Interp<'_> {
    /// `w^{s..} |= p` (F.5.3.1) with `depth` approximation levels left.
    fn sat(&self, p: &P, w: &Word, s: usize, depth: usize) -> bool {
        match p {
            // strong(R): some nonempty prefix tightly satisfies R. On the
            // finite word of a whole simulation (decision A13-D7), a strong
            // obligation whose first clock never ticks imposes nothing.
            P::Strong(r, clock) => {
                !ends(r, w, s).lasts.is_empty()
                    || (w.tail.is_none()
                        && (s..w.len()).all(|i| w.letter(i).is_some_and(|l| !clock.holds(l))))
            }
            // weak(R): every prefix followed by T^ω satisfies strong(R).
            P::Weak(r) => self.prefixes(w, s).all(|letters| {
                let top = Word {
                    letters: &letters,
                    tail: Some(if w.dual { Letter::Bot } else { Letter::Top }),
                    dual: w.dual,
                };
                !ends(r, &top, s).lasts.is_empty()
            }),
            P::Not(p) => !self.sat(p, &w.dualized(), s, depth),
            P::And(x, y) => self.sat(x, w, s, depth) && self.sat(y, w, s, depth),
            P::Or(x, y) => self.sat(x, w, s, depth) || self.sat(y, w, s, depth),
            P::Imp(r, p) => ends(r, w, s)
                .lasts
                .iter()
                .all(|last| self.sat(p, w, *last, depth)),
            // nexttime P: |w| = 0 or w^{1..} |= P.
            P::Next(p) => {
                if !w.starts().contains(&s) {
                    true
                } else {
                    let next = w.after(s);
                    self.sat(p, w, next, depth)
                }
            }
            P::Until(x, y) => {
                let starts: Vec<usize> = (s..w.starts().end).collect();
                starts.iter().enumerate().any(|(k, j)| {
                    self.sat(y, w, *j, depth)
                        && starts[..k].iter().all(|i| self.sat(x, w, *i, depth))
                }) || starts.iter().all(|i| self.sat(x, w, *i, depth))
            }
            // accept_on(b) P: w |= P, or some letter i satisfies b and
            // w_{0,i-1} T^ω |= P.
            P::Accept(b, p) => {
                self.sat(p, w, s, depth)
                    || self.abort_points(w, s).into_iter().any(|(letters, at)| {
                        let top = Word {
                            letters: &letters[..at],
                            tail: Some(if w.dual { Letter::Bot } else { Letter::Top }),
                            dual: w.dual,
                        };
                        w.letter_at(&letters, at)
                            .is_some_and(|letter| b.holds(letter))
                            && self.sat(p, &top, s, depth)
                    })
            }
            P::Call(index) => {
                let key = (*index, depth, s, w.letters.to_vec(), w.tail, w.dual);
                if let Some(known) = self.memo.borrow().get(&key) {
                    return *known;
                }
                let result = if depth == 0 {
                    self.sat(&self.calls.zero[*index], w, s, 0)
                } else {
                    self.sat(&self.calls.bodies[*index], w, s, depth - 1)
                };
                self.memo.borrow_mut().insert(key, result);
                result
            }
        }
    }

    /// The raw letters of `w_{0,j}` for every `j >= s` the word has, with up
    /// to [`TAIL_LOOKAHEAD`] letters of an infinite tail.
    fn prefixes(&self, w: &Word, s: usize) -> impl Iterator<Item = Vec<Letter>> {
        let raw = w.letters.to_vec();
        let tail = w.tail;
        let end = w.len();
        let extra = if tail.is_some() { TAIL_LOOKAHEAD } else { 0 };
        (s..end + extra).map(move |j| {
            let mut letters = raw.clone();
            while letters.len() <= j {
                letters.push(tail.expect("tail letters exist past the finite part"));
            }
            letters.truncate(j + 1);
            letters
        })
    }

    /// Candidate abort letters `i >= s` with the raw letters up to and
    /// including `i`.
    fn abort_points(&self, w: &Word, s: usize) -> Vec<(Vec<Letter>, usize)> {
        self.prefixes(w, s)
            .map(|letters| {
                let at = letters.len() - 1;
                (letters, at)
            })
            .collect()
    }
}

impl Word<'_> {
    /// The letter at `i` of `letters` in this word's view.
    fn letter_at(&self, letters: &[Letter], i: usize) -> Option<Letter> {
        let raw = letters.get(i).copied()?;
        Some(if self.dual { raw.dual() } else { raw })
    }
}

/// The report of one evaluation attempt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Outcome {
    /// Passed or failed on the letter with this index.
    Pass(usize),
    Fail(usize),
    /// Undecided at the end, with a strong obligation unmet (fails at
    /// `$finish`, S38-D1).
    FailAtEnd,
    /// Undecided at the end with only weak obligations: no report.
    Pending,
}

/// Letters that can follow a prefix: every combination of the sampled
/// values with at least one clock ticking.
fn alphabet() -> Vec<Letter> {
    let mut letters = Vec::new();
    for (c1, c2) in [(true, false), (false, true), (true, true)] {
        for a in [false, true] {
            for b in [false, true] {
                letters.push(Letter::V { a, b, c1, c2 });
            }
        }
    }
    letters
}

pub(super) struct Oracle<'p> {
    interp: Interp<'p>,
    top: P,
    alphabet: Vec<Letter>,
    cache: HashMap<Vec<Letter>, Option<bool>>,
}

impl<'p> Oracle<'p> {
    /// The oracle of `@(posedge c1) q` in `program`.
    pub(super) fn new(program: &'p Program, q: &Q) -> Self {
        let mut calls = Calls {
            program,
            keys: Vec::new(),
            bodies: Vec::new(),
            zero: Vec::new(),
        };
        let top = calls.t_p(q, &Cond::C1);
        Oracle {
            interp: Interp {
                calls,
                memo: Default::default(),
            },
            top,
            alphabet: alphabet(),
            cache: HashMap::new(),
        }
    }

    fn holds(&self, w: &Word) -> bool {
        self.interp.memo.borrow_mut().clear();
        self.interp.sat(&self.top, w, 0, depth_for(w))
    }

    /// Whether the prefix `u` already decides the property: `Some(result)`
    /// when every continuation `u v c^ω` (`|v| <= 1`, `v` and `c` real
    /// letters) gives the same result. Two continuations that differ prove
    /// the prefix undecided; the bounded set can only call a prefix decided
    /// too early, which the comparison with the simulator would expose.
    fn decide(&mut self, u: &[Letter]) -> Option<bool> {
        if let Some(known) = self.cache.get(u) {
            return *known;
        }
        let mut seen = [false, false];
        let mut word = u.to_vec();
        'outer: for v in std::iter::once(None).chain(self.alphabet.iter().copied().map(Some)) {
            if let Some(v) = v {
                word.push(v);
            }
            for tail in &self.alphabet {
                let w = Word {
                    letters: &word,
                    tail: Some(*tail),
                    dual: false,
                };
                seen[usize::from(self.holds(&w))] = true;
                if seen == [true, true] {
                    if v.is_some() {
                        word.pop();
                    }
                    break 'outer;
                }
            }
            if v.is_some() {
                word.pop();
            }
        }
        let decision = match seen {
            [false, true] => Some(true),
            [true, false] => Some(false),
            _ => None,
        };
        self.cache.insert(u.to_vec(), decision);
        decision
    }

    /// The outcome of the attempt that starts on the first letter of
    /// `trace`, which ends at `$finish`.
    pub(super) fn outcome(&mut self, trace: &[Letter]) -> Outcome {
        for k in 0..trace.len() {
            match self.decide(&trace[..=k]) {
                Some(true) => return Outcome::Pass(k),
                Some(false) => return Outcome::Fail(k),
                None => {}
            }
        }
        // F.5.3.1 on the finite word: a strong obligation needs its match
        // inside the word, a weak one does not.
        let w = Word {
            letters: trace,
            tail: None,
            dual: false,
        };
        if self.holds(&w) {
            Outcome::Pending
        } else {
            Outcome::FailAtEnd
        }
    }
}

// Builders for the fixture's properties.

pub(super) fn a() -> Cond {
    Cond::A
}

pub(super) fn b() -> Cond {
    Cond::B
}

pub(super) fn nb() -> Cond {
    not(Cond::B)
}

pub(super) fn one() -> Cond {
    Cond::One
}

pub(super) fn c2() -> Cond {
    Cond::C2
}

pub(super) fn c1() -> Cond {
    Cond::C1
}

pub(super) fn sb(b: Cond) -> S {
    S::Bool(b)
}

pub(super) fn cat(x: S, y: S) -> S {
    S::Cat(bx(x), bx(y))
}

pub(super) fn fuse(x: S, y: S) -> S {
    S::Fuse(bx(x), bx(y))
}

pub(super) fn at(c: Cond, s: S) -> S {
    S::Clock(c, bx(s))
}

pub(super) fn qat(c: Cond, q: Q) -> Q {
    Q::Clock(c, bx(q))
}

pub(super) fn bxq(q: Q) -> Box<Q> {
    bx(q)
}
