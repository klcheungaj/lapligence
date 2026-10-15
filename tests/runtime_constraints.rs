//! Standalone tests for the constraint solver runtime (`llg_constraint.c`).
//!
//! A deterministic generator builds small random problems over every node
//! operation, with guards, state leaves, inactive variables, disabled blocks
//! and function calls. An independent Rust evaluator enumerates every
//! assignment (SIM-029 acceptance A01): the solver's status must match
//! (satisfiable, unsatisfiable or evaluation error), every sample must be a
//! solution, and exact-engine samples must pass a chi-square bound. Forced
//! budgets drive the rejection and search fallbacks and the resource status
//! (A03). SIM-030 adds random divisors, indices and `**` here and the
//! weighted, ordered and randc probes in `runtime_constraints/weighted.rs`.

use crate::{c_compiler, sim_harness};

mod expand;
mod overflow;
mod weighted;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::Command;
use std::time::Duration;

// Operation codes, mirroring llg_constraint.h.
const CONST: u16 = 1;
const VAR: u16 = 2;
const STATE: u16 = 3;
const CALL: u16 = 4;
const RESIZE: u16 = 5;
const NOT: u16 = 6;
const NEG: u16 = 7;
const REDAND: u16 = 8;
const REDOR: u16 = 9;
const REDXOR: u16 = 10;
const REDNAND: u16 = 11;
const REDNOR: u16 = 12;
const REDXNOR: u16 = 13;
const LNOT: u16 = 14;
const ADD: u16 = 15;
const SUB: u16 = 16;
const MUL: u16 = 17;
const DIV: u16 = 18;
const MOD: u16 = 19;
const AND: u16 = 20;
const OR: u16 = 21;
const XOR: u16 = 22;
const XNOR: u16 = 23;
const SHL: u16 = 24;
const SHR: u16 = 25;
const ASHR: u16 = 26;
const EQ: u16 = 27;
const NE: u16 = 28;
const LT: u16 = 29;
const LE: u16 = 30;
const GT: u16 = 31;
const GE: u16 = 32;
const LAND: u16 = 33;
const LOR: u16 = 34;
const IMPL: u16 = 35;
const EQUIV: u16 = 36;
const COND: u16 = 37;
const CONCAT: u16 = 38;
const REPL: u16 = 39;
const SLICE: u16 = 40;
const DSLICE: u16 = 41;
const ALL: u16 = 42;
const POW: u16 = 43;
const DIST: u16 = 44;

const INPUT_KNOWN: u8 = 0;
const INPUT_NULL: u8 = 1;
const INPUT_XZ: u8 = 2;

#[derive(Clone, Copy, Debug)]
struct Node {
    op: u16,
    signed: bool,
    width: u32,
    a: u32,
    b: u32,
    c: u32,
}

#[derive(Clone, Debug)]
struct Var {
    width: u32,
    signed: bool,
    domain: Option<u32>,
    active: bool,
    input: u8,
    current: u128,
    /// `randc` (18.4.2).
    cyclic: bool,
}

#[derive(Clone, Debug)]
struct Leaf {
    width: u32,
    signed: bool,
    input: u8,
    value: u128,
}

#[derive(Clone, Debug)]
struct CallSite {
    width: u32,
    signed: bool,
    args: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
struct Problem {
    nodes: Vec<Node>,
    args: Vec<u32>,
    consts: Vec<u64>,
    vars: Vec<Var>,
    blocks: Vec<(u32, bool)>,
    leaves: Vec<Leaf>,
    calls: Vec<CallSite>,
    /// Some call has a random argument: the layered order (18.5.11) makes
    /// the outcome differ from the flat enumeration, so only membership of
    /// successful samples is checked.
    random_call: bool,
    dists: Vec<Dist>,
    /// (block, before, after) solve-before orders (18.5.9).
    orders: Vec<(u32, u32, u32)>,
}

/// One `dist` constraint (`llg_cr_dist_t`).
#[derive(Clone, Debug)]
struct Dist {
    expr: u32,
    guard: Option<u32>,
    items: Vec<DistItem>,
}

#[derive(Clone, Debug)]
struct DistItem {
    matches: u32,
    lo: u32,
    hi: u32,
    weight: Option<u32>,
    per_range: bool,
    signed: bool,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }
}

fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

fn signed_value(value: u128, width: u32) -> i128 {
    let value = value & mask(width);
    if width > 0 && width < 128 && (value >> (width - 1)) & 1 == 1 {
        (value | !mask(width)) as i128
    } else {
        value as i128
    }
}

fn extend(value: u128, from: u32, signed: bool, to: u32) -> u128 {
    let v = if signed {
        signed_value(value, from) as u128
    } else {
        value & mask(from)
    };
    v & mask(to)
}

// ---- generator ----------------------------------------------------------

struct Gen<'a> {
    rng: &'a mut Rng,
    p: Problem,
    allow_calls: bool,
    wide: bool,
    /// Random divisors, select indices and `**` (SIM-030, S30-D4).
    extended: bool,
}

impl Gen<'_> {
    fn push(&mut self, op: u16, signed: bool, width: u32, a: u32, b: u32, c: u32) -> u32 {
        self.p.nodes.push(Node {
            op,
            signed,
            width,
            a,
            b,
            c,
        });
        (self.p.nodes.len() - 1) as u32
    }

    fn constant(&mut self, width: u32, signed: bool, value: u128) -> u32 {
        let first = self.p.consts.len() as u32;
        let value = value & mask(width);
        self.p.consts.push(value as u64);
        if width > 64 {
            self.p.consts.push((value >> 64) as u64);
        }
        self.push(CONST, signed, width, first, 0, 0)
    }

    fn max_concat(&self) -> u32 {
        if self.wide {
            120
        } else {
            24
        }
    }

    fn node(&self, n: u32) -> Node {
        self.p.nodes[n as usize]
    }

    fn resize(&mut self, n: u32, width: u32, signed: bool) -> u32 {
        let node = self.node(n);
        if node.width == width && node.signed == signed {
            return n;
        }
        self.push(RESIZE, signed, width, n, 0, 0)
    }

    /// An expression without random variables (constants and state leaves).
    fn known(&mut self, depth: u32) -> u32 {
        if depth == 0 || self.rng.chance(1, 2) {
            if self.rng.chance(1, 3) && !self.p.leaves.is_empty() {
                let leaf = self.rng.below(self.p.leaves.len() as u64) as u32;
                let l = self.p.leaves[leaf as usize].clone();
                return self.push(STATE, l.signed, l.width, leaf, 0, 0);
            }
            let width = 1 + self.rng.below(if self.wide { 70 } else { 4 }) as u32;
            let signed = self.rng.chance(1, 3);
            let value = if self.wide && self.rng.chance(1, 2) {
                self.rng.below(9) as u128
            } else {
                ((self.rng.next() as u128) << 64) | self.rng.next() as u128
            };
            return self.constant(width, signed, value);
        }
        let a = self.known(depth - 1);
        let b = self.known(depth - 1);
        let na = self.node(a);
        let nb = self.node(b);
        let w = na.width.max(nb.width);
        let s = na.signed && nb.signed;
        let a = self.resize(a, w, s);
        let b = self.resize(b, w, s);
        let op = [ADD, SUB, XOR, AND][self.rng.below(4) as usize];
        self.push(op, s, w, a, b, 0)
    }

    fn expr(&mut self, depth: u32) -> u32 {
        if depth == 0 || self.rng.chance(1, 4) {
            let choice = self.rng.below(10);
            if choice < 6 {
                let v = self.rng.below(self.p.vars.len() as u64) as u32;
                let var = self.p.vars[v as usize].clone();
                return self.push(VAR, var.signed, var.width, v, 0, 0);
            }
            if choice == 9 && self.allow_calls && self.p.calls.len() < 3 {
                let arg = if self.rng.chance(1, 2) {
                    let v = self.rng.below(self.p.vars.len() as u64) as u32;
                    let var = self.p.vars[v as usize].clone();
                    self.p.random_call |= var.active;
                    self.push(VAR, var.signed, var.width, v, 0, 0)
                } else {
                    self.known(1)
                };
                let width = 1 + self.rng.below(4) as u32;
                let signed = self.rng.chance(1, 3);
                let call = self.p.calls.len() as u32;
                self.p.calls.push(CallSite {
                    width,
                    signed,
                    args: vec![arg],
                });
                return self.push(CALL, signed, width, call, 0, 0);
            }
            return self.known(1);
        }
        if self.extended && self.rng.chance(1, 8) {
            let a = self.expr(depth - 1);
            let b = if self.rng.chance(1, 2) {
                self.expr(depth - 1)
            } else {
                self.known(1)
            };
            let na = self.node(a);
            return self.push(POW, na.signed, na.width, a, b, 0);
        }
        let op = self.rng.below(38);
        match op {
            0 => {
                let a = self.expr(depth - 1);
                let width = 1 + self.rng.below(5) as u32;
                let signed = self.rng.chance(1, 2);
                self.push(RESIZE, signed, width, a, 0, 0)
            }
            1 | 2 => {
                let a = self.expr(depth - 1);
                let na = self.node(a);
                self.push([NOT, NEG][(op - 1) as usize], na.signed, na.width, a, 0, 0)
            }
            3..=8 => {
                let a = self.expr(depth - 1);
                let red = [REDAND, REDOR, REDXOR, REDNAND, REDNOR, REDXNOR][(op - 3) as usize];
                self.push(red, false, 1, a, 0, 0)
            }
            9 => {
                let a = self.expr(depth - 1);
                self.push(LNOT, false, 1, a, 0, 0)
            }
            10..=16 => {
                let a = self.expr(depth - 1);
                let b = self.expr(depth - 1);
                let na = self.node(a);
                let nb = self.node(b);
                let w = na.width.max(nb.width);
                let s = na.signed && nb.signed;
                let a = self.resize(a, w, s);
                let b = self.resize(b, w, s);
                let code = [ADD, SUB, MUL, AND, OR, XOR, XNOR][(op - 10) as usize];
                self.push(code, s, w, a, b, 0)
            }
            17 | 18 => {
                let a = self.expr(depth - 1);
                let b = if self.extended && self.rng.chance(1, 2) {
                    self.expr(depth - 1)
                } else {
                    self.known(1)
                };
                let na = self.node(a);
                let nb = self.node(b);
                let w = na.width.max(nb.width);
                let s = na.signed && nb.signed;
                let a = self.resize(a, w, s);
                let b = self.resize(b, w, s);
                self.push([DIV, MOD][(op - 17) as usize], s, w, a, b, 0)
            }
            19..=21 => {
                let a = self.expr(depth - 1);
                let b = self.expr(depth - 1);
                let na = self.node(a);
                self.push(
                    [SHL, SHR, ASHR][(op - 19) as usize],
                    na.signed,
                    na.width,
                    a,
                    b,
                    0,
                )
            }
            22..=27 => {
                let a = self.expr(depth - 1);
                let b = self.expr(depth - 1);
                if self.rng.chance(1, 2) {
                    let na = self.node(a);
                    let nb = self.node(b);
                    let w = na.width.max(nb.width);
                    let s = na.signed && nb.signed;
                    let a = self.resize(a, w, s);
                    let b = self.resize(b, w, s);
                    self.push(
                        [EQ, NE, LT, LE, GT, GE][(op - 22) as usize],
                        false,
                        1,
                        a,
                        b,
                        0,
                    )
                } else {
                    self.push(
                        [EQ, NE, LT, LE, GT, GE][(op - 22) as usize],
                        false,
                        1,
                        a,
                        b,
                        0,
                    )
                }
            }
            28..=31 => {
                let a = self.expr(depth - 1);
                let b = self.expr(depth - 1);
                self.push(
                    [LAND, LOR, IMPL, EQUIV][(op - 28) as usize],
                    false,
                    1,
                    a,
                    b,
                    0,
                )
            }
            32 => {
                let c = self.expr(depth - 1);
                let a = self.expr(depth - 1);
                let b = self.expr(depth - 1);
                let na = self.node(a);
                let nb = self.node(b);
                let w = na.width.max(nb.width);
                let s = na.signed && nb.signed;
                let a = self.resize(a, w, s);
                let b = self.resize(b, w, s);
                self.push(COND, s, w, c, a, b)
            }
            33 => {
                let count = 2 + self.rng.below(2) as u32;
                let mut items = Vec::new();
                let mut width = 0;
                for _ in 0..count {
                    let x = self.expr(depth - 1);
                    width += self.node(x).width;
                    items.push(x);
                }
                if width > self.max_concat() {
                    return items[0];
                }
                let first = self.p.args.len() as u32;
                self.p.args.extend(items);
                self.push(CONCAT, false, width, 0, first, count)
            }
            34 => {
                let a = self.expr(depth - 1);
                let count = 1 + self.rng.below(3) as u32;
                let width = self.node(a).width * count;
                if width > self.max_concat() {
                    return a;
                }
                self.push(REPL, false, width, a, count, 0)
            }
            35 => {
                let a = self.expr(depth - 1);
                let aw = self.node(a).width;
                let lsb = self.rng.below(aw as u64) as u32;
                let width = 1 + self.rng.below((aw - lsb) as u64) as u32;
                self.push(SLICE, false, width, a, lsb, 0)
            }
            36 => {
                let a = self.expr(depth - 1);
                let aw = self.node(a).width;
                let width = 1 + self.rng.below(aw as u64) as u32;
                let index = if self.extended && self.rng.chance(1, 2) {
                    self.expr(depth - 1)
                } else {
                    self.known(1)
                };
                self.push(DSLICE, false, width, a, index, 0)
            }
            _ => {
                let count = 2 + self.rng.below(2) as u32;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(self.expr(depth - 1));
                }
                let first = self.p.args.len() as u32;
                self.p.args.extend(items);
                self.push(ALL, false, 1, 0, first, count)
            }
        }
    }
}

fn generate(seed: u64, allow_calls: bool, wide: bool) -> Problem {
    generate_with(seed, allow_calls, wide, false)
}

fn generate_with(seed: u64, allow_calls: bool, wide: bool, extended: bool) -> Problem {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    for _ in 0..4 {
        rng.next();
    }
    let mut p = Problem::default();
    let var_count = 1 + rng.below(3) as usize;
    let mut bits = 0;
    for _ in 0..var_count {
        let width = 1 + rng.below(if wide { 100 } else { 5 }) as u32;
        if !wide && bits + width > 10 {
            break;
        }
        bits += width;
        let active = !rng.chance(1, 6);
        let input = if !active && rng.chance(1, 4) {
            INPUT_XZ
        } else {
            INPUT_KNOWN
        };
        p.vars.push(Var {
            width,
            signed: rng.chance(1, 2),
            domain: None,
            active,
            input,
            current: (((rng.next() as u128) << 64) | rng.next() as u128) & mask(width),
            cyclic: false,
        });
    }
    if p.vars.is_empty() {
        p.vars.push(Var {
            width: 3,
            signed: false,
            domain: None,
            active: true,
            input: INPUT_KNOWN,
            current: 0,
            cyclic: false,
        });
    }
    for _ in 0..rng.below(3) {
        let width = 1 + rng.below(if wide { 70 } else { 4 }) as u32;
        let input = match rng.below(10) {
            0 => INPUT_NULL,
            1 => INPUT_XZ,
            _ => INPUT_KNOWN,
        };
        p.leaves.push(Leaf {
            width,
            signed: rng.chance(1, 2),
            input,
            value: (((rng.next() as u128) << 64) | rng.next() as u128) & mask(width),
        });
    }
    let mut g = Gen {
        rng: &mut rng,
        p,
        allow_calls,
        wide,
        extended,
    };
    // An enum-like domain on the first variable sometimes.
    if g.rng.chance(1, 5) {
        let v = 0u32;
        let var = g.p.vars[0].clone();
        let x = g.push(VAR, var.signed, var.width, v, 0, 0);
        let mut ors = Vec::new();
        for _ in 0..1 + g.rng.below(3) {
            let value = g.rng.next() as u128;
            let c = g.constant(var.width, var.signed, value);
            ors.push(g.push(EQ, false, 1, x, c, 0));
        }
        let mut acc = ors[0];
        for &o in &ors[1..] {
            acc = g.push(LOR, false, 1, acc, o, 0);
        }
        g.p.vars[0].domain = Some(acc);
    }
    let blocks = 1 + g.rng.below(3);
    for _ in 0..blocks {
        let depth = 1 + g.rng.below(3) as u32;
        let root = g.expr(depth);
        let enabled = !g.rng.chance(1, 5);
        g.p.blocks.push((root, enabled));
    }
    g.p
}

// ---- independent evaluator ----------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum St {
    K(u128),
    R,
    E,
}

fn call_result(call: u32, args: &[u128], width: u32) -> (u128, u8) {
    let mut sum: u128 = 1 + call as u128;
    for &a in args {
        sum = sum.wrapping_add(a.wrapping_mul(3));
    }
    let input = if args.first().is_some_and(|&a| a % 7 == 6) {
        INPUT_XZ
    } else {
        INPUT_KNOWN
    };
    (sum & mask(width), input)
}

/// Classify every node; `assignment` gives values for active variables that
/// are known (None = still random).
fn classify(p: &Problem, assignment: &[Option<u128>]) -> Vec<St> {
    let mut st: Vec<St> = Vec::with_capacity(p.nodes.len());
    let truth = |s: St| match s {
        St::K(v) => Some(v != 0),
        _ => None,
    };
    for (i, n) in p.nodes.iter().enumerate() {
        let get = |x: u32| st[x as usize];
        let w = n.width;
        let val_of = |x: u32, to: u32| -> u128 {
            let nx = p.nodes[x as usize];
            match st[x as usize] {
                St::K(v) => extend(v, nx.width, nx.signed, to),
                _ => unreachable!(),
            }
        };
        let generic = |ops: &[u32]| -> Option<St> {
            if ops.iter().any(|&x| get(x) == St::E) {
                return Some(St::E);
            }
            if ops.iter().any(|&x| get(x) == St::R) {
                return Some(St::R);
            }
            None
        };
        let s = match n.op {
            CONST => St::K(p.consts[n.a as usize] as u128 & mask(w)),
            VAR => {
                let var = &p.vars[n.a as usize];
                if var.active {
                    match assignment[n.a as usize] {
                        Some(v) => St::K(extend(v, var.width, var.signed, w)),
                        None => St::R,
                    }
                } else if var.input != INPUT_KNOWN {
                    St::E
                } else {
                    St::K(extend(var.current, var.width, var.signed, w))
                }
            }
            STATE => {
                let leaf = &p.leaves[n.a as usize];
                if leaf.input != INPUT_KNOWN {
                    St::E
                } else {
                    St::K(extend(leaf.value, leaf.width, leaf.signed, w))
                }
            }
            CALL => {
                let call = &p.calls[n.a as usize];
                match generic(&call.args) {
                    Some(s) => s,
                    None => {
                        let args: Vec<u128> = call
                            .args
                            .iter()
                            .map(|&x| val_of(x, p.nodes[x as usize].width))
                            .collect();
                        let (v, input) = call_result(n.a, &args, call.width);
                        if input != INPUT_KNOWN {
                            St::E
                        } else {
                            St::K(extend(v, call.width, call.signed, w))
                        }
                    }
                }
            }
            DIST => {
                // Membership in an item of nonzero weight (18.5.4), from the
                // bounds and weights directly rather than the match nodes.
                let d = &p.dists[n.a as usize];
                match get(d.expr) {
                    St::E => St::E,
                    St::R => St::R,
                    St::K(_) => St::K(dist_member(p, d, &val_of) as u128),
                }
            }
            LAND | LOR | IMPL | ALL => {
                let (ops, invert): (Vec<u32>, Vec<bool>) = match n.op {
                    ALL => (
                        p.args[n.b as usize..(n.b + n.c) as usize].to_vec(),
                        vec![false; n.c as usize],
                    ),
                    IMPL => (vec![n.a, n.b], vec![true, false]),
                    _ => (vec![n.a, n.b], vec![false, false]),
                };
                let disj = n.op == LOR || n.op == IMPL;
                let mut dominant = false;
                let mut error = false;
                let mut random = false;
                for (k, &x) in ops.iter().enumerate() {
                    match get(x) {
                        St::K(_) => {
                            let t = truth(get(x)).unwrap() ^ invert[k];
                            if t == disj {
                                dominant = true;
                            }
                        }
                        St::E => error = true,
                        St::R => random = true,
                    }
                }
                if dominant {
                    St::K(disj as u128)
                } else if error {
                    St::E
                } else if random {
                    St::R
                } else {
                    St::K(!disj as u128)
                }
            }
            _ => {
                let ops: Vec<u32> = match n.op {
                    CONCAT => p.args[n.b as usize..(n.b + n.c) as usize].to_vec(),
                    REPL | SLICE | RESIZE | NOT | NEG | REDAND | REDOR | REDXOR | REDNAND
                    | REDNOR | REDXNOR | LNOT => vec![n.a],
                    COND => vec![n.a, n.b, n.c],
                    _ => vec![n.a, n.b],
                };
                if let Some(s) = generic(&ops) {
                    if s == St::R && (n.op == DIV || n.op == MOD) {
                        let nb = p.nodes[n.b as usize];
                        match get(n.b) {
                            St::K(v) if extend(v, nb.width, nb.signed, w) == 0 => St::E,
                            _ => St::R,
                        }
                    } else if s == St::R && n.op == DSLICE {
                        match get(n.b) {
                            St::K(v) => {
                                let nb = p.nodes[n.b as usize];
                                let idx = if nb.signed {
                                    signed_value(v, nb.width)
                                } else {
                                    v as i128
                                };
                                let aw = p.nodes[n.a as usize].width as i128;
                                if idx < 0 || idx + w as i128 > aw {
                                    St::E
                                } else {
                                    St::R
                                }
                            }
                            // A random index is checked per assignment (S30-D4).
                            _ => St::R,
                        }
                    } else {
                        s
                    }
                } else {
                    eval_op(p, n, &val_of)
                }
            }
        };
        let _ = i;
        st.push(s);
    }
    st
}

/// Items of `d` holding its expression's current value, with their
/// per-value weights (w for `:=`, w / range size for `:/`).
fn dist_matches(p: &Problem, d: &Dist, val_of: &dyn Fn(u32, u32) -> u128) -> Vec<f64> {
    let ne = p.nodes[d.expr as usize];
    let value = val_of(d.expr, ne.width);
    let mut weights = Vec::new();
    for item in &d.items {
        let lo = val_of(item.lo, ne.width);
        let hi = val_of(item.hi, ne.width);
        let (v, lo, hi) = if item.signed {
            (
                signed_value(value, ne.width),
                signed_value(lo, ne.width),
                signed_value(hi, ne.width),
            )
        } else {
            (value as i128, lo as i128, hi as i128)
        };
        let weight = item
            .weight
            .map_or(1, |w| val_of(w, p.nodes[w as usize].width)) as f64;
        if weight == 0.0 || v < lo || v > hi {
            continue;
        }
        weights.push(if item.per_range {
            weight / (hi - lo + 1) as f64
        } else {
            weight
        });
    }
    weights
}

fn dist_member(p: &Problem, d: &Dist, val_of: &dyn Fn(u32, u32) -> u128) -> bool {
    !dist_matches(p, d, val_of).is_empty()
}

fn eval_op(p: &Problem, n: &Node, val_of: &dyn Fn(u32, u32) -> u128) -> St {
    let w = n.width;
    let m = mask(w);
    let own = |x: u32| val_of(x, p.nodes[x as usize].width);
    let bit = |b: bool| St::K(b as u128);
    match n.op {
        RESIZE => St::K(val_of(n.a, w)),
        NOT => St::K(!val_of(n.a, w) & m),
        NEG => St::K(val_of(n.a, w).wrapping_neg() & m),
        REDAND => bit(own(n.a) == mask(p.nodes[n.a as usize].width)),
        REDNAND => bit(own(n.a) != mask(p.nodes[n.a as usize].width)),
        REDOR => bit(own(n.a) != 0),
        REDNOR => bit(own(n.a) == 0),
        REDXOR => bit(own(n.a).count_ones() % 2 == 1),
        REDXNOR => bit(own(n.a).count_ones() % 2 == 0),
        LNOT => bit(own(n.a) == 0),
        EQUIV => bit((own(n.a) != 0) == (own(n.b) != 0)),
        ADD => St::K(val_of(n.a, w).wrapping_add(val_of(n.b, w)) & m),
        SUB => St::K(val_of(n.a, w).wrapping_sub(val_of(n.b, w)) & m),
        MUL => St::K(val_of(n.a, w).wrapping_mul(val_of(n.b, w)) & m),
        AND => St::K(val_of(n.a, w) & val_of(n.b, w)),
        OR => St::K(val_of(n.a, w) | val_of(n.b, w)),
        XOR => St::K(val_of(n.a, w) ^ val_of(n.b, w)),
        XNOR => St::K(!(val_of(n.a, w) ^ val_of(n.b, w)) & m),
        DIV | MOD => {
            let a = val_of(n.a, w);
            let b = val_of(n.b, w);
            if b == 0 {
                return St::E;
            }
            if n.signed {
                let (sa, sb) = (signed_value(a, w), signed_value(b, w));
                let r = if n.op == DIV {
                    sa.wrapping_div(sb)
                } else {
                    sa.wrapping_rem(sb)
                };
                St::K(r as u128 & m)
            } else {
                St::K(if n.op == DIV { a / b } else { a % b })
            }
        }
        SHL | SHR | ASHR => {
            let a = val_of(n.a, w);
            let amount = own(n.b);
            let neg = n.signed && w > 0 && (a >> (w - 1)) & 1 == 1;
            if n.op == SHL {
                St::K(if amount >= w as u128 {
                    0
                } else {
                    (a << amount) & m
                })
            } else if n.op == ASHR && neg {
                St::K(if amount >= w as u128 {
                    m
                } else {
                    (signed_value(a, w) >> amount) as u128 & m
                })
            } else {
                St::K(if amount >= w as u128 { 0 } else { a >> amount })
            }
        }
        EQ | NE | LT | LE | GT | GE => {
            let na = p.nodes[n.a as usize];
            let nb = p.nodes[n.b as usize];
            let cw = na.width.max(nb.width);
            let a = val_of(n.a, cw);
            let b = val_of(n.b, cw);
            let ord = if na.signed && nb.signed {
                signed_value(a, cw).cmp(&signed_value(b, cw))
            } else {
                a.cmp(&b)
            };
            use std::cmp::Ordering::*;
            bit(match n.op {
                EQ => ord == Equal,
                NE => ord != Equal,
                LT => ord == Less,
                LE => ord != Greater,
                GT => ord == Greater,
                _ => ord != Less,
            })
        }
        COND => {
            let pick = if own(n.a) != 0 { n.b } else { n.c };
            St::K(val_of(pick, w))
        }
        CONCAT => {
            let mut v: u128 = 0;
            for &x in &p.args[n.b as usize..(n.b + n.c) as usize] {
                let xw = p.nodes[x as usize].width;
                v = (v << xw) | own(x);
            }
            St::K(v & m)
        }
        REPL => {
            let aw = p.nodes[n.a as usize].width;
            let mut v: u128 = 0;
            for _ in 0..n.b {
                v = (v << aw) | own(n.a);
            }
            St::K(v & m)
        }
        SLICE => St::K((own(n.a) >> n.b) & m),
        POW => {
            // Table 11-4 on the node's width; the exponent is self-determined.
            let nb = p.nodes[n.b as usize];
            let a = val_of(n.a, w);
            let b = own(n.b);
            let b_negative = nb.signed && (b >> (nb.width - 1)) & 1 == 1;
            let a_value = if n.signed {
                signed_value(a, w)
            } else {
                a as i128
            };
            if b == 0 {
                St::K(1 & m)
            } else if b_negative {
                match a_value {
                    0 => St::E,
                    1 => St::K(1),
                    -1 => St::K(if b & 1 == 1 { m } else { 1 }),
                    _ => St::K(0),
                }
            } else {
                // Repeated multiplication modulo 2^w: independent of the
                // solver's square-and-multiply.
                assert!(
                    b < 1 << 20,
                    "exponent {b} too large for the probe evaluator"
                );
                let mut acc: u128 = 1 & m;
                for _ in 0..b {
                    acc = acc.wrapping_mul(a) & m;
                }
                St::K(acc & m)
            }
        }
        DSLICE => {
            let nb = p.nodes[n.b as usize];
            let idx = if nb.signed {
                signed_value(own(n.b), nb.width)
            } else {
                own(n.b) as i128
            };
            let aw = p.nodes[n.a as usize].width as i128;
            if idx < 0 || idx + w as i128 > aw {
                St::E
            } else {
                St::K((own(n.a) >> idx) & m)
            }
        }
        other => panic!("unexpected op {other}"),
    }
}

fn roots(p: &Problem) -> Vec<u32> {
    let mut r: Vec<u32> = p
        .blocks
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(root, _)| *root)
        .collect();
    for v in &p.vars {
        if v.active {
            if let Some(d) = v.domain {
                r.push(d);
            }
        }
    }
    r
}

#[derive(Debug, PartialEq)]
enum Expect {
    Error,
    Unsat,
    Sat(Vec<Vec<u128>>),
}

/// Enumerate the active variables' assignments, mirroring the solver's
/// contract (no layering: problems with random call arguments are only
/// checked for membership).
fn expected(p: &Problem) -> Expect {
    let active: Vec<usize> = (0..p.vars.len()).filter(|&v| p.vars[v].active).collect();
    let free: Vec<Option<u128>> = vec![None; p.vars.len()];
    let st = classify(p, &free);
    let rs = roots(p);
    if rs.iter().any(|&r| st[r as usize] == St::E) {
        return Expect::Error;
    }
    if rs.iter().any(|&r| matches!(st[r as usize], St::K(0))) {
        return Expect::Unsat;
    }
    let total_bits: u32 = active.iter().map(|&v| p.vars[v].width).sum();
    let mut solutions = Vec::new();
    for code in 0u128..(1u128 << total_bits) {
        let mut assignment = vec![None; p.vars.len()];
        let mut rest = code;
        for &v in &active {
            let w = p.vars[v].width;
            assignment[v] = Some(rest & mask(w));
            rest >>= w;
        }
        if satisfies(p, &assignment) {
            solutions.push(active.iter().map(|&v| assignment[v].unwrap()).collect());
        }
    }
    if solutions.is_empty() {
        Expect::Unsat
    } else {
        Expect::Sat(solutions)
    }
}

fn satisfies(p: &Problem, assignment: &[Option<u128>]) -> bool {
    let st = classify(p, assignment);
    roots(p)
        .iter()
        .all(|&r| matches!(st[r as usize], St::K(v) if v != 0))
}

// ---- C emission -----------------------------------------------------------

fn emit_problem(out: &mut String, index: usize, p: &Problem) {
    let name = format!("p{index}");
    writeln!(out, "static const llg_cr_node_t {name}_nodes[] = {{").unwrap();
    for n in &p.nodes {
        writeln!(
            out,
            "    {{{}, {}, 0, {}, {}u, {}u, {}u}},",
            n.op, n.signed as u8, n.width, n.a, n.b, n.c
        )
        .unwrap();
    }
    writeln!(out, "}};").unwrap();
    let list = |v: Vec<String>| {
        if v.is_empty() {
            "0".to_owned()
        } else {
            v.join(", ")
        }
    };
    writeln!(
        out,
        "static const uint64_t {name}_consts[] = {{{}}};",
        list(p.consts.iter().map(|c| format!("UINT64_C({c})")).collect())
    )
    .unwrap();
    writeln!(out, "static const llg_cr_var_t {name}_vars[] = {{").unwrap();
    for (i, v) in p.vars.iter().enumerate() {
        writeln!(
            out,
            "    {{\"v{i}\", {}, {}, {}, {}}},",
            v.width,
            v.signed as u8,
            v.domain
                .map_or("LLG_CR_NONE".to_owned(), |d| format!("{d}u")),
            v.cyclic as u8
        )
        .unwrap();
    }
    writeln!(out, "}};").unwrap();
    writeln!(out, "static const llg_cr_block_t {name}_blocks[] = {{").unwrap();
    for (i, (root, _)) in p.blocks.iter().enumerate() {
        writeln!(out, "    {{\"b{i}\", {root}u}},").unwrap();
    }
    writeln!(out, "}};").unwrap();
    let leaves: Vec<String> = p
        .leaves
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{{\"l{i}\", {}, {}}}", l.width, l.signed as u8))
        .collect();
    writeln!(
        out,
        "static const llg_cr_leaf_t {name}_leaves[] = {{{}}};",
        if leaves.is_empty() {
            "{0, 0, 0}".to_owned()
        } else {
            leaves.join(", ")
        }
    )
    .unwrap();
    let mut first = 0u32;
    let mut call_args: Vec<u32> = Vec::new();
    let calls: Vec<String> = p
        .calls
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let s = format!(
                "{{\"c{i}\", {}, {}, {}u, {}u}}",
                c.width,
                c.signed as u8,
                p.args.len() as u32 + first,
                c.args.len()
            );
            first += c.args.len() as u32;
            call_args.extend(&c.args);
            s
        })
        .collect();
    // Calls read their arguments after the shared operand list.
    let mut all_args = p.args.clone();
    all_args.extend(call_args);
    writeln!(
        out,
        "static const uint32_t {name}_all_args[] = {{{}}};",
        list(all_args.iter().map(|a| format!("{a}u")).collect())
    )
    .unwrap();
    writeln!(
        out,
        "static const llg_cr_call_t {name}_calls[] = {{{}}};",
        if calls.is_empty() {
            "{0, 0, 0, 0, 0}".to_owned()
        } else {
            calls.join(", ")
        }
    )
    .unwrap();
    let optional = |n: Option<u32>| n.map_or("LLG_CR_NONE".to_owned(), |n| format!("{n}u"));
    let mut dists = Vec::new();
    let mut items = Vec::new();
    for d in &p.dists {
        dists.push(format!(
            "{{{}u, {}, {}u, {}u}}",
            d.expr,
            optional(d.guard),
            items.len(),
            d.items.len()
        ));
        for item in &d.items {
            items.push(format!(
                "{{{}u, {}u, {}u, {}, {}, {}}}",
                item.matches,
                item.lo,
                item.hi,
                optional(item.weight),
                item.per_range as u8,
                item.signed as u8
            ));
        }
    }
    let orders: Vec<String> = p
        .orders
        .iter()
        .map(|(block, before, after)| format!("{{{block}u, {before}u, {after}u}}"))
        .collect();
    let table = |v: &[String], empty: &str| {
        if v.is_empty() {
            empty.to_owned()
        } else {
            v.join(", ")
        }
    };
    writeln!(
        out,
        "static const llg_cr_dist_t {name}_dists[] = {{{}}};\nstatic const llg_cr_dist_item_t {name}_items[] = {{{}}};\nstatic const llg_cr_order_t {name}_orders[] = {{{}}};",
        table(&dists, "{0, 0, 0, 0}"),
        table(&items, "{0, 0, 0, 0, 0, 0}"),
        table(&orders, "{0, 0, 0}")
    )
    .unwrap();
    writeln!(
        out,
        "static const llg_cr_problem_t {name} = {{\"{name}\", {name}_nodes, {}, {name}_all_args, {}, {name}_consts, {}, {name}_vars, {}, {name}_blocks, {}, {name}_leaves, {}, {name}_calls, {}, {name}_dists, {}, {name}_items, {}, {name}_orders, {}, NULL, 0, NULL, 0, NULL, 0, NULL, 0, NULL, 0, NULL, 0}};",
        p.nodes.len(),
        all_args.len(),
        p.consts.len(),
        p.vars.len(),
        p.blocks.len(),
        p.leaves.len(),
        p.calls.len(),
        dists.len(),
        items.len(),
        orders.len()
    )
    .unwrap();
    writeln!(out, "static void {name}_inputs(llg_cr_session_t* s) {{").unwrap();
    for (i, v) in p.vars.iter().enumerate() {
        writeln!(
            out,
            "    {{ uint64_t w[2] = {{UINT64_C({}), UINT64_C({})}}; llg_cr_set_var(s, {i}, w, {}, {}); }}",
            v.current as u64, (v.current >> 64) as u64, v.active as u8, v.input
        )
        .unwrap();
    }
    for (i, (_, enabled)) in p.blocks.iter().enumerate() {
        writeln!(out, "    llg_cr_set_block(s, {i}, {});", *enabled as u8).unwrap();
    }
    for (i, l) in p.leaves.iter().enumerate() {
        writeln!(
            out,
            "    {{ uint64_t w[2] = {{UINT64_C({}), UINT64_C({})}}; llg_cr_set_leaf(s, {i}, w, {}); }}",
            l.value as u64, (l.value >> 64) as u64, l.input
        )
        .unwrap();
    }
    writeln!(out, "}}").unwrap();
}

const PROBE_MAIN: &str = r#"
static const llg_cr_problem_t* current;

static int thunk(void* context, uint32_t call, const uint64_t* const* args,
                 uint64_t* result) {
    (void)context;
    const llg_cr_call_t* c = &current->calls[call];
    uint64_t sum = 1u + call;
    for (uint32_t i = 0; i < c->arg_count; ++i) sum += args[i][0] * 3u;
    result[0] = c->bit_width >= 64 ? sum : (sum & ((UINT64_C(1) << c->bit_width) - 1u));
    return (c->arg_count && args[0][0] % 7u == 6u) ? 2 : 0;
}

static uint32_t next_draw(void* stream) {
    uint64_t* x = (uint64_t*)stream;
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    return (uint32_t)(*x >> 32);
}

int main(int argc, char** argv) {
    uint32_t samples = argc > 1 ? (uint32_t)atoi(argv[1]) : 64u;
    static llg_cr_session_t* slots[PROBLEM_COUNT];
    for (uint32_t i = 0; i < PROBLEM_COUNT; ++i) {
        current = problems[i];
        uint64_t stream = UINT64_C(0x9e3779b97f4a7c15) ^ ((uint64_t)i * 7919u + 1u);
        llg_cr_rng_t rng = {next_draw, &stream};
        llg_cr_session_t* s = llg_cr_session_get(problems[i], &slots[i]);
        for (uint32_t k = 0; k < samples; ++k) {
            inputs[i](s);
            llg_cr_report_t report;
            llg_cr_status_t status = llg_cr_solve(s, &rng, thunk, NULL, &report);
            printf("P%u %d", i, (int)status);
            if (status == LLG_CR_OK) {
                for (uint32_t v = 0; v < problems[i]->var_count; ++v)
                {
                    const uint64_t* w = llg_cr_value(s, v);
                    unsigned long long hi = llg_cr_var_words(s, v) > 1u ? (unsigned long long)w[1] : 0ull;
                    printf(" %016llx%016llx", hi, (unsigned long long)w[0]);
                }
                printf(" e%u", report.engines);
            } else if (status == LLG_CR_ERROR) {
                printf(" r%d i%u", report.reason, report.item);
            } else if (status == LLG_CR_RESOURCE) {
                printf(" %s", report.budget);
            }
            printf("\n");
            if (status != LLG_CR_OK) break;
        }
    }
    llg_cr_session_release_all();
    return 0;
}
"#;

fn constraint_sources() -> (String, String) {
    let (header, source) = llg::sim::rt::constraint_sources();
    (header.to_owned(), source.to_owned())
}

fn build_probe(
    problems: &[Problem],
    name: &str,
) -> Option<(sim_harness::TempDir, std::path::PathBuf)> {
    build_probe_main(problems, name, PROBE_MAIN)
}

fn build_probe_main(
    problems: &[Problem],
    name: &str,
    main: &str,
) -> Option<(sim_harness::TempDir, std::path::PathBuf)> {
    let mut probe = String::from(
        "#include \"llg_constraint.h\"\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n",
    );
    for (i, p) in problems.iter().enumerate() {
        emit_problem(&mut probe, i, p);
    }
    writeln!(probe, "#define PROBLEM_COUNT {}u", problems.len()).unwrap();
    writeln!(probe, "static const llg_cr_problem_t* problems[] = {{").unwrap();
    for i in 0..problems.len() {
        writeln!(probe, "    &p{i},").unwrap();
    }
    writeln!(probe, "}};").unwrap();
    writeln!(probe, "static void (*inputs[])(llg_cr_session_t*) = {{").unwrap();
    for i in 0..problems.len() {
        writeln!(probe, "    p{i}_inputs,").unwrap();
    }
    writeln!(probe, "}};").unwrap();
    probe.push_str(main);
    build_c_probe(name, &probe)
}

/// Compile `probe` (a complete C file) with the solver as strict C11.
fn build_c_probe(name: &str, probe: &str) -> Option<(sim_harness::TempDir, std::path::PathBuf)> {
    let compiler = c_compiler::host_c_compiler();
    if !c_compiler::c_compiler_available(&compiler) {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return None;
    }
    let dir = sim_harness::TempDir::new(name).expect("create constraint probe directory");
    let (header, source) = constraint_sources();
    std::fs::write(dir.path().join("llg_constraint.h"), header).unwrap();
    std::fs::write(dir.path().join("llg_constraint.c"), source).unwrap();
    std::fs::write(dir.path().join("probe.c"), probe).unwrap();
    let (mut command, executable) = c_compiler::strict_c11_executable(
        &compiler,
        dir.path(),
        c_compiler::Optimize::Speed,
        &["llg_constraint.c", "probe.c"],
        "constraint_probe",
    );
    let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
        .unwrap_or_else(|error| panic!("compile constraint probe: {error}"));
    assert!(
        output.status.success(),
        "constraint probe must compile:\n{}",
        c_compiler::compiler_diagnostics(&output)
    );
    Some((dir, executable))
}

fn run_probe(executable: &std::path::Path, samples: u32, env: &[(&str, &str)]) -> String {
    let mut command = Command::new(executable);
    command.arg(samples.to_string());
    run_command_output(&mut command, env)
}

/// Run a probe with the solver tunables cleared, then `env` applied.
fn run_command_output(command: &mut Command, env: &[(&str, &str)]) -> String {
    for name in [
        "LLG_RAND_BDD_NODES",
        "LLG_RAND_BDD_BITS",
        "LLG_RAND_REJECT_TRIES",
        "LLG_RAND_CONFLICTS",
        "LLG_RAND_ARRAY_ELEMENTS",
        "LLG_RAND_EXPAND_NODES",
        "LLG_RAND_STATS",
        "LLG_RAND_OVERFLOW_MEMO",
    ] {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let output = sim_harness::run_command(command, Duration::from_secs(300))
        .unwrap_or_else(|error| panic!("run constraint probe: {error}"));
    assert!(
        output.status.success(),
        "constraint probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[derive(Debug, Default)]
struct Outcome {
    statuses: Vec<i32>,
    samples: Vec<Vec<u128>>,
    engines: u32,
    detail: String,
}

fn parse(output: &str, count: usize) -> Vec<Outcome> {
    let mut outcomes: Vec<Outcome> = (0..count).map(|_| Outcome::default()).collect();
    for line in output.lines() {
        let mut fields = line.split_whitespace();
        let index: usize = fields.next().unwrap()[1..].parse().unwrap();
        let status: i32 = fields.next().unwrap().parse().unwrap();
        let o = &mut outcomes[index];
        o.statuses.push(status);
        let rest: Vec<&str> = fields.collect();
        if status == 0 {
            let mut values = Vec::new();
            for f in rest {
                if let Some(e) = f.strip_prefix('e') {
                    o.engines |= e.parse::<u32>().unwrap();
                } else {
                    values.push(u128::from_str_radix(f, 16).unwrap());
                }
            }
            o.samples.push(values);
        } else {
            o.detail = rest.join(" ");
        }
    }
    outcomes
}

/// Chi-square bound with df = k-1; generous enough that a fixed-seed correct
/// sampler never fails while a biased one (e.g. per-branch choice) does.
fn chi_square_ok(counts: &BTreeMap<Vec<u128>, u64>, k: usize, n: u64) -> bool {
    if k < 2 {
        return true;
    }
    let expected = n as f64 / k as f64;
    let mut chi = 0.0;
    for count in counts.values() {
        let d = *count as f64 - expected;
        chi += d * d / expected;
    }
    chi += (k - counts.len()) as f64 * expected;
    let df = (k - 1) as f64;
    chi <= df + 6.0 * (2.0 * df).sqrt() + 10.0
}

enum Mode {
    Exact,
    Fallback,
    /// Only the CDCL search runs: it must decide every small problem
    /// exactly, like the exact engine, but its samples are not uniform.
    Search,
}

fn check(problems: &[Problem], outcomes: &[Outcome], mode: Mode) -> (usize, usize, usize) {
    let (mut sat, mut unsat, mut error) = (0, 0, 0);
    for (i, (p, o)) in problems.iter().zip(outcomes).enumerate() {
        let active: Vec<usize> = (0..p.vars.len()).filter(|&v| p.vars[v].active).collect();
        let last = *o.statuses.last().expect("every problem prints a status");
        // Membership of every successful sample, against the full classifier.
        for sample in &o.samples {
            let mut assignment = vec![None; p.vars.len()];
            for &v in &active {
                assignment[v] = Some(sample[v]);
            }
            assert!(
                satisfies(p, &assignment),
                "problem {i}: sample {sample:?} violates the constraints\n{p:#?}"
            );
            for (var, value) in p.vars.iter().zip(sample) {
                // Solutions never carry bits above the variable's width.
                if var.active {
                    assert_eq!(value & !mask(var.width), 0, "problem {i}");
                }
            }
        }
        if p.random_call {
            continue;
        }
        let expect = expected(p);
        match (&expect, last, &mode) {
            (Expect::Error, 3, _) => error += 1,
            (Expect::Unsat, 1, _) => unsat += 1,
            (Expect::Unsat, 2, Mode::Fallback) => unsat += 1,
            (Expect::Sat(_), 2, Mode::Fallback) => {}
            (Expect::Sat(solutions), 0, _) => {
                sat += 1;
                if matches!(mode, Mode::Exact) && o.engines & 0x8 == 0 && solutions.len() <= 32 {
                    let mut counts: BTreeMap<Vec<u128>, u64> = BTreeMap::new();
                    for sample in &o.samples {
                        let key: Vec<u128> = active.iter().map(|&v| sample[v]).collect();
                        assert!(solutions.contains(&key), "problem {i}");
                        *counts.entry(key).or_default() += 1;
                    }
                    assert!(
                        chi_square_ok(&counts, solutions.len(), o.samples.len() as u64),
                        "problem {i}: sample distribution is not uniform over {} solutions: {counts:?}",
                        solutions.len()
                    );
                }
            }
            _ => panic!(
                "problem {i}: expected {}, solver returned status {last} ({}); problem {p:#?}",
                match &expect {
                    Expect::Error => "error".to_owned(),
                    Expect::Unsat => "unsat".to_owned(),
                    Expect::Sat(s) => format!("sat with {} solutions", s.len()),
                },
                o.detail
            ),
        }
    }
    (sat, unsat, error)
}

/// SIM-030 (S30-D4): random divisors, random select indices and `**` are
/// random operations whose evaluation errors exclude only the assignments
/// that raise them, with Figure 18-3 absorption by a dominating operand.
#[test]
fn constraint_solver_handles_random_divisors_indices_and_power() {
    let problems: Vec<Problem> = (0..400u64)
        .map(|seed| generate_with(seed + 20_000, false, false, true))
        .collect();
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-extended") else {
        return;
    };
    let output = run_probe(&executable, 400, &[]);
    let outcomes = parse(&output, problems.len());
    let (sat, unsat, error) = check(&problems, &outcomes, Mode::Exact);
    eprintln!("extended exhaustive: sat={sat} unsat={unsat} error={error}");
    assert!(
        sat > 100 && unsat > 10,
        "sat={sat} unsat={unsat} error={error}"
    );
    let pow = problems
        .iter()
        .filter(|p| p.nodes.iter().any(|n| n.op == POW))
        .count();
    assert!(pow > 40, "only {pow} problems use **");

    let output = run_probe(&executable, 40, &[("LLG_RAND_BDD_NODES", "1")]);
    let outcomes = parse(&output, problems.len());
    check(&problems, &outcomes, Mode::Fallback);
    let search_only = [("LLG_RAND_BDD_BITS", "0"), ("LLG_RAND_REJECT_TRIES", "0")];
    let output = run_probe(&executable, 40, &search_only);
    let outcomes = parse(&output, problems.len());
    check(&problems, &outcomes, Mode::Search);
}

#[test]
fn constraint_solver_matches_an_exhaustive_enumerator_on_small_problems() {
    let problems: Vec<Problem> = (0..400u64)
        .map(|seed| generate(seed, seed % 4 == 3, false))
        .collect();
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-exhaustive") else {
        return;
    };
    let output = run_probe(&executable, 400, &[]);
    let outcomes = parse(&output, problems.len());
    let (sat, unsat, error) = check(&problems, &outcomes, Mode::Exact);
    eprintln!("exhaustive: sat={sat} unsat={unsat} error={error}");
    // The generator must exercise every outcome.
    assert!(
        sat > 100 && unsat > 10 && error > 10,
        "sat={sat} unsat={unsat} error={error}"
    );

    // Forced fallbacks: no exact engine, so only rejection (and search)
    // remain; outcomes stay sound and resource exhaustion is distinguishable.
    let output = run_probe(&executable, 40, &[("LLG_RAND_BDD_NODES", "1")]);
    let outcomes = parse(&output, problems.len());
    let (sat, unsat, error) = check(&problems, &outcomes, Mode::Fallback);
    eprintln!("fallback: sat={sat} unsat={unsat} error={error}");

    // Search only: no component fits the exact engine's zero-bit bound and
    // rejection is off, so the CDCL fallback alone decides every constrained
    // component. It must find a solution or prove UNSAT exactly as the
    // enumerator says; its samples need not be uniform (S29-D10) but must vary.
    let search_only = [("LLG_RAND_BDD_BITS", "0"), ("LLG_RAND_REJECT_TRIES", "0")];
    let output = run_probe(&executable, 40, &search_only);
    let outcomes = parse(&output, problems.len());
    let (sat, unsat, error) = check(&problems, &outcomes, Mode::Search);
    let (mut searched, mut several, mut varied) = (0, 0, 0);
    for (p, o) in problems.iter().zip(&outcomes) {
        if o.engines & 0x8 == 0 || p.random_call {
            continue;
        }
        searched += 1;
        if let Expect::Sat(solutions) = expected(p) {
            if solutions.len() >= 4 {
                several += 1;
                let distinct: std::collections::BTreeSet<&Vec<u128>> = o.samples.iter().collect();
                if distinct.len() > 1 {
                    varied += 1;
                }
            }
        }
    }
    eprintln!(
        "search: sat={sat} unsat={unsat} error={error} searched={searched} \
         varied={varied}/{several}"
    );
    assert!(
        searched > 100 && unsat > 10,
        "searched={searched} unsat={unsat}"
    );
    assert!(varied * 10 >= several * 9, "varied={varied}/{several}");

    // A one-conflict budget: whatever the search cannot finish is a resource
    // outcome naming the conflict budget, never a wrong status (A03).
    let output = run_probe(
        &executable,
        4,
        &[
            ("LLG_RAND_BDD_BITS", "0"),
            ("LLG_RAND_REJECT_TRIES", "0"),
            ("LLG_RAND_CONFLICTS", "1"),
        ],
    );
    let outcomes = parse(&output, problems.len());
    check(&problems, &outcomes, Mode::Fallback);
    let resource: Vec<&str> = outcomes
        .iter()
        .filter(|o| o.statuses.last() == Some(&2))
        .map(|o| o.detail.as_str())
        .collect();
    eprintln!("one conflict: resource={}", resource.len());
    assert!(!resource.is_empty());
    assert!(
        resource
            .iter()
            .all(|budget| *budget == "LLG_RAND_CONFLICTS"),
        "{resource:?}"
    );
}

/// Wide operands (multi-word values, up to 100-bit variables): too large to
/// enumerate, so every sample is checked for membership and statuses for
/// soundness against the guard classification.
#[test]
fn constraint_solver_samples_wide_problems_soundly() {
    let problems: Vec<Problem> = (0..200u64)
        .map(|seed| generate(seed + 10_000, false, true))
        .collect();
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-wide") else {
        return;
    };
    let output = run_probe(&executable, 8, &[]);
    let outcomes = parse(&output, problems.len());
    let mut sat = 0;
    for (i, (p, o)) in problems.iter().zip(&outcomes).enumerate() {
        let active: Vec<usize> = (0..p.vars.len()).filter(|&v| p.vars[v].active).collect();
        for sample in &o.samples {
            let mut assignment = vec![None; p.vars.len()];
            for &v in &active {
                assignment[v] = Some(sample[v]);
            }
            assert!(
                satisfies(p, &assignment),
                "problem {i}: {sample:x?}\n{p:#?}"
            );
        }
        let free = vec![None; p.vars.len()];
        let st = classify(p, &free);
        let rs = roots(p);
        let last = *o.statuses.last().unwrap();
        if rs.iter().any(|&r| st[r as usize] == St::E) {
            assert_eq!(last, 3, "problem {i}: expected an evaluation error");
        } else if rs.iter().any(|&r| matches!(st[r as usize], St::K(0))) {
            assert_eq!(last, 1, "problem {i}: expected unsat");
        } else {
            assert_ne!(last, 3, "problem {i}: unexpected error {}", o.detail);
            if last == 0 {
                sat += 1;
            }
        }
    }
    eprintln!("wide: sat={sat}");
    assert!(sat > 50, "sat={sat}");
}
