//! SIM-030 acceptance for the solver runtime: `dist` weights, `solve...before`
//! orders and `randc` cycles.
//!
//! A01: generated small problems with weighted items, guards, disabled
//! blocks and orders are enumerated by an independent evaluator that
//! computes each solution's probability under the layered weighted measure
//! (decisions S30-D1 and S30-D2; a dist whose guard holds weighs a legal
//! combination by its weights' density relative to uniform over its
//! support, |S| * w(v) / W): the solver's samples must stay inside the
//! solutions and pass a chi-square bound fixed before the run. Larger
//! seeded problems check the LRM examples against predeclared tolerances.
//!
//! A02 (runtime part): `randc` never repeats a value within a cycle, keeps
//! one cycle per object and variable, and starts a new cycle when its
//! constraints change (S30-D3), on the exact and the direct paths.

use super::*;
use std::collections::BTreeSet;

fn var_node(g: &mut Gen, v: u32) -> u32 {
    let var = g.p.vars[v as usize].clone();
    g.push(VAR, var.signed, var.width, v, 0, 0)
}

fn new_var(width: u32, signed: bool) -> Var {
    Var {
        width,
        signed,
        domain: None,
        active: true,
        input: INPUT_KNOWN,
        current: 0,
        cyclic: false,
    }
}

/// Item `lo..=hi` (a single value when equal) of a dist on `expr`.
fn dist_item(
    g: &mut Gen,
    expr: u32,
    lo: u128,
    hi: u128,
    weight: Option<u128>,
    per_range: bool,
) -> DistItem {
    let ne = g.node(expr);
    let lo_node = g.constant(ne.width, ne.signed, lo);
    let hi_node = g.constant(ne.width, ne.signed, hi);
    let matches = if lo == hi {
        g.push(EQ, false, 1, expr, lo_node, 0)
    } else {
        let ge = g.push(GE, false, 1, expr, lo_node, 0);
        let le = g.push(LE, false, 1, expr, hi_node, 0);
        g.push(LAND, false, 1, ge, le, 0)
    };
    let weight = weight.map(|w| g.constant(16, false, w));
    DistItem {
        matches,
        lo: lo_node,
        hi: hi_node,
        weight,
        per_range,
        signed: ne.signed,
    }
}

/// Add `dist` on `expr` (under `guard`) as its own block; returns the block.
fn add_dist(
    g: &mut Gen,
    expr: u32,
    guard: Option<u32>,
    items: Vec<DistItem>,
    enabled: bool,
) -> u32 {
    let index = g.p.dists.len() as u32;
    g.p.dists.push(Dist { expr, guard, items });
    let node = g.push(DIST, false, 1, index, 0, 0);
    let root = match guard {
        Some(guard) => g.push(IMPL, false, 1, guard, node, 0),
        None => node,
    };
    g.p.blocks.push((root, enabled));
    g.p.blocks.len() as u32 - 1
}

fn weighted_problem(seed: u64) -> Problem {
    let mut rng = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) | 1);
    for _ in 0..4 {
        rng.next();
    }
    let mut p = Problem::default();
    let count = 2 + rng.below(2) as usize;
    for _ in 0..count {
        let width = 1 + rng.below(3) as u32;
        p.vars.push(new_var(width, rng.chance(1, 2)));
    }
    let mut g = Gen {
        rng: &mut rng,
        p,
        allow_calls: false,
        wide: false,
        extended: false,
    };
    if g.rng.chance(2, 3) {
        let depth = 1 + g.rng.below(2) as u32;
        let root = g.expr(depth);
        g.p.blocks.push((root, true));
    }
    for _ in 0..1 + g.rng.below(2) {
        let a = g.rng.below(count as u64) as u32;
        let mut expr = var_node(&mut g, a);
        if g.rng.chance(1, 3) {
            let b = g.rng.below(count as u64) as u32;
            let other = var_node(&mut g, b);
            let (na, nb) = (g.node(expr), g.node(other));
            let width = na.width.max(nb.width) + 1;
            let signed = na.signed && nb.signed;
            let x = g.resize(expr, width, signed);
            let y = g.resize(other, width, signed);
            expr = g.push(ADD, signed, width, x, y, 0);
        }
        let guard = if g.rng.chance(1, 3) {
            let v = g.rng.below(count as u64) as u32;
            let x = var_node(&mut g, v);
            let op = [REDOR, REDXOR][g.rng.below(2) as usize];
            Some(g.push(op, false, 1, x, 0, 0))
        } else {
            None
        };
        let ne = g.node(expr);
        let span = 1u64 << ne.width;
        let mut items = Vec::new();
        for _ in 0..1 + g.rng.below(3) {
            let x = g.rng.below(span) as u128;
            let y = if g.rng.chance(1, 2) {
                x
            } else {
                g.rng.below(span) as u128
            };
            let key = |v: u128| {
                if ne.signed {
                    signed_value(v, ne.width)
                } else {
                    v as i128
                }
            };
            let (lo, hi) = if key(x) <= key(y) { (x, y) } else { (y, x) };
            let weight = if g.rng.chance(1, 4) {
                None
            } else {
                Some(g.rng.below(5) as u128)
            };
            let per_range = g.rng.chance(1, 2);
            items.push(dist_item(&mut g, expr, lo, hi, weight, per_range));
        }
        let enabled = !g.rng.chance(1, 6);
        add_dist(&mut g, expr, guard, items, enabled);
    }
    if g.rng.chance(2, 3) {
        // Orders follow a random rank, so they never form a cycle.
        let mut rank: Vec<u64> = (0..count).map(|_| g.rng.next()).collect();
        rank.dedup();
        for _ in 0..1 + g.rng.below(2) {
            let i = g.rng.below(count as u64) as usize;
            let j = g.rng.below(count as u64) as usize;
            if i == j || rank.len() != count {
                continue;
            }
            let (before, after) = if rank[i] < rank[j] { (i, j) } else { (j, i) };
            let block = g.rng.below(g.p.blocks.len() as u64) as u32;
            g.p.orders.push((block, before as u32, after as u32));
        }
    }
    g.p
}

/// Variables a node reads, through every operand.
fn node_support(p: &Problem, n: u32, out: &mut BTreeSet<u32>) {
    let node = p.nodes[n as usize];
    let operands: Vec<u32> = match node.op {
        CONST | STATE => Vec::new(),
        VAR => {
            out.insert(node.a);
            Vec::new()
        }
        CALL => p.calls[node.a as usize].args.clone(),
        CONCAT | ALL => p.args[node.b as usize..(node.b + node.c) as usize].to_vec(),
        DIST => {
            let d = &p.dists[node.a as usize];
            let mut v = vec![d.expr];
            v.extend(d.guard);
            for item in &d.items {
                v.extend([item.matches, item.lo, item.hi]);
                v.extend(item.weight);
            }
            v
        }
        RESIZE | NOT | NEG | REDAND | REDOR | REDXOR | REDNAND | REDNOR | REDXNOR | LNOT | REPL
        | SLICE => vec![node.a],
        COND => vec![node.a, node.b, node.c],
        _ => vec![node.a, node.b],
    };
    for x in operands {
        node_support(p, x, out);
    }
}

/// DIST nodes reachable from the enabled blocks.
fn live_dists(p: &Problem) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<u32> = roots(p);
    let mut dists = Vec::new();
    while let Some(n) = stack.pop() {
        if !seen.insert(n) {
            continue;
        }
        let node = p.nodes[n as usize];
        match node.op {
            CONST | VAR | STATE => {}
            CALL => stack.extend(&p.calls[node.a as usize].args),
            CONCAT | ALL => stack.extend(&p.args[node.b as usize..(node.b + node.c) as usize]),
            DIST => {
                dists.push(n);
                stack.push(p.dists[node.a as usize].expr);
            }
            RESIZE | NOT | NEG | REDAND | REDOR | REDXOR | REDNAND | REDNOR | REDXNOR | LNOT
            | REPL | SLICE => stack.push(node.a),
            COND => stack.extend([node.a, node.b, node.c]),
            _ => stack.extend([node.a, node.b]),
        }
    }
    dists
}

/// Layer of each variable under S30-D2: orders from enabled blocks between
/// active non-randc variables give heights (longest path to a sink); larger
/// heights solve first and unordered variables join the last set.
fn layers(p: &Problem) -> Vec<usize> {
    let live: Vec<(u32, u32)> = p
        .orders
        .iter()
        .filter(|(block, before, after)| {
            p.blocks[*block as usize].1
                && before != after
                && p.vars[*before as usize].active
                && p.vars[*after as usize].active
                && !p.vars[*before as usize].cyclic
                && !p.vars[*after as usize].cyclic
        })
        .map(|(_, before, after)| (*before, *after))
        .collect();
    fn height(v: u32, live: &[(u32, u32)], depth: usize) -> usize {
        assert!(depth < 64, "circular order in a generated problem");
        live.iter()
            .filter(|(before, _)| *before == v)
            .map(|(_, after)| height(*after, live, depth + 1) + 1)
            .max()
            .unwrap_or(0)
    }
    let heights: Vec<usize> = (0..p.vars.len() as u32)
        .map(|v| height(v, &live, 0))
        .collect();
    let mut distinct: Vec<usize> = heights.clone();
    distinct.sort_unstable_by(|a, b| b.cmp(a));
    distinct.dedup();
    heights
        .iter()
        .map(|h| distinct.iter().position(|d| d == h).unwrap())
        .collect()
}

/// S30-D1 weight of a dist whose guard holds: the density of its weights
/// relative to uniform over its support, |S| * w(v) / W.
fn dist_density(p: &Problem, d: &Dist, val_of: &dyn Fn(u32, u32) -> u128) -> f64 {
    let width = p.nodes[d.expr as usize].width;
    let mut intervals = Vec::new();
    let mut mass = 0.0;
    for item in &d.items {
        let weight = item
            .weight
            .map_or(1, |w| val_of(w, p.nodes[w as usize].width)) as f64;
        if weight == 0.0 {
            continue;
        }
        let bound = |n: u32| {
            let v = val_of(n, width);
            if item.signed {
                signed_value(v, width)
            } else {
                v as i128
            }
        };
        let (lo, hi) = (bound(item.lo), bound(item.hi));
        if hi < lo {
            continue;
        }
        mass += if item.per_range {
            weight
        } else {
            weight * (hi - lo + 1) as f64
        };
        intervals.push((lo, hi));
    }
    intervals.sort_unstable();
    let mut support = 0i128;
    let mut current: Option<(i128, i128)> = None;
    for (lo, hi) in intervals {
        current = match current {
            Some((cl, ch)) if lo <= ch => Some((cl, ch.max(hi))),
            Some((cl, ch)) => {
                support += ch - cl + 1;
                Some((lo, hi))
            }
            None => Some((lo, hi)),
        };
    }
    if let Some((cl, ch)) = current {
        support += ch - cl + 1;
    }
    let matched: f64 = dist_matches(p, d, val_of).iter().sum();
    support as f64 * matched / mass
}

/// Probability of every solution under the layered weighted measure, or
/// None when the problem is not satisfiable.
fn weighted_expectation(p: &Problem) -> Option<BTreeMap<Vec<u128>, f64>> {
    let Expect::Sat(solutions) = expected(p) else {
        return None;
    };
    let active: Vec<usize> = (0..p.vars.len()).filter(|&v| p.vars[v].active).collect();
    let layer = layers(p);
    let layer_count = active.iter().map(|&v| layer[v]).max().unwrap_or(0) + 1;
    // Each live dist weighs the layer of its latest variable.
    let dists: Vec<(u32, usize)> = live_dists(p)
        .into_iter()
        .map(|n| {
            let mut support = BTreeSet::new();
            node_support(p, n, &mut support);
            let l = support
                .iter()
                .map(|&v| layer[v as usize])
                .max()
                .unwrap_or(0);
            (p.nodes[n as usize].a, l)
        })
        .collect();
    let weight = |solution: &[u128], l: usize| -> f64 {
        let mut assignment = vec![None; p.vars.len()];
        for (k, &v) in active.iter().enumerate() {
            assignment[v] = Some(solution[k]);
        }
        let st = classify(p, &assignment);
        let val_of = |x: u32, to: u32| -> u128 {
            let nx = p.nodes[x as usize];
            match st[x as usize] {
                St::K(v) => extend(v, nx.width, nx.signed, to),
                other => panic!("node {x} is {other:?} under a full assignment"),
            }
        };
        let mut w = 1.0;
        for &(d, dl) in &dists {
            if dl != l {
                continue;
            }
            let dist = &p.dists[d as usize];
            if dist.guard.is_some_and(|g| st[g as usize] == St::K(0)) {
                continue;
            }
            w *= dist_density(p, dist, &val_of);
        }
        w
    };
    let project = |solution: &[u128], upto: usize| -> Vec<Option<u128>> {
        active
            .iter()
            .enumerate()
            .map(|(k, &v)| (layer[v] <= upto).then_some(solution[k]))
            .collect()
    };
    let mut result = BTreeMap::new();
    for x in &solutions {
        let mut probability = 1.0;
        for l in 0..layer_count {
            let earlier = if l == 0 {
                vec![None; active.len()]
            } else {
                project(x, l - 1)
            };
            // Distinct values of this layer that extend x's earlier layers.
            let mut options: BTreeMap<Vec<Option<u128>>, f64> = BTreeMap::new();
            for y in &solutions {
                let matches_prefix = l == 0 || project(y, l - 1) == earlier;
                if matches_prefix {
                    options.entry(project(y, l)).or_insert_with(|| weight(y, l));
                }
            }
            let total: f64 = options.values().sum();
            probability *= options[&project(x, l)] / total;
        }
        result.insert(x.clone(), probability);
    }
    Some(result)
}

/// Pearson chi-square against `expected` probabilities, pooling cells
/// below 5 expected samples; the bound df + 7*sqrt(2 df) + 12 is fixed in
/// advance (a false failure has probability below about 1e-6 per problem).
fn weighted_chi_square_ok(
    counts: &BTreeMap<Vec<u128>, u64>,
    expected: &BTreeMap<Vec<u128>, f64>,
    n: u64,
) -> (bool, f64) {
    let mut cells: Vec<(f64, f64)> = Vec::new();
    let (mut pooled_e, mut pooled_o) = (0.0, 0.0);
    for (key, p) in expected {
        let e = p * n as f64;
        let o = *counts.get(key).unwrap_or(&0) as f64;
        if e < 5.0 {
            pooled_e += e;
            pooled_o += o;
        } else {
            cells.push((e, o));
        }
    }
    if pooled_e > 0.0 {
        cells.push((pooled_e, pooled_o));
    }
    if cells.len() < 2 {
        return (true, 0.0);
    }
    let chi: f64 = cells.iter().map(|(e, o)| (o - e) * (o - e) / e).sum();
    let df = (cells.len() - 1) as f64;
    (chi <= df + 7.0 * (2.0 * df).sqrt() + 12.0, chi)
}

#[test]
fn constraint_solver_samples_dist_and_solve_before_by_the_layered_weighted_measure() {
    let mut problems = Vec::new();
    let mut expectations = Vec::new();
    for seed in 0..4000u64 {
        if problems.len() == 60 {
            break;
        }
        let p = weighted_problem(seed);
        let Some(expectation) = weighted_expectation(&p) else {
            continue;
        };
        if !(2..=64).contains(&expectation.len()) {
            continue;
        }
        problems.push(p);
        expectations.push(expectation);
    }
    assert_eq!(
        problems.len(),
        60,
        "the generator must yield enough problems"
    );
    let skewed = expectations
        .iter()
        .filter(|e| {
            let max = e.values().cloned().fold(0.0, f64::max);
            let min = e.values().cloned().fold(1.0, f64::min);
            max > 1.5 * min
        })
        .count();
    let ordered = problems.iter().filter(|p| !p.orders.is_empty()).count();
    assert!(
        skewed >= 20 && ordered >= 20,
        "skewed={skewed} ordered={ordered}"
    );
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-weighted") else {
        return;
    };
    let samples = 6000u32;
    let output = run_probe(&executable, samples, &[]);
    let outcomes = parse(&output, problems.len());
    for (i, ((p, o), expectation)) in problems
        .iter()
        .zip(&outcomes)
        .zip(&expectations)
        .enumerate()
    {
        assert_eq!(
            o.statuses.last(),
            Some(&0),
            "problem {i}: {} {p:#?}",
            o.detail
        );
        assert_eq!(o.samples.len(), samples as usize, "problem {i}");
        let active: Vec<usize> = (0..p.vars.len()).filter(|&v| p.vars[v].active).collect();
        let mut counts: BTreeMap<Vec<u128>, u64> = BTreeMap::new();
        for sample in &o.samples {
            let key: Vec<u128> = active.iter().map(|&v| sample[v]).collect();
            let probability = expectation.get(&key).copied().unwrap_or(0.0);
            assert!(
                probability > 0.0,
                "problem {i}: sample {key:?} has probability 0\n{p:#?}"
            );
            *counts.entry(key).or_default() += 1;
        }
        let (ok, chi) = weighted_chi_square_ok(&counts, expectation, samples as u64);
        assert!(
            ok,
            "problem {i}: chi-square {chi:.1} over {} solutions\nexpected {expectation:?}\ncounts {counts:?}\n{p:#?}",
            expectation.len()
        );
    }
}

/// Fraction `hits / n` within five standard deviations of `p` (fixed
/// before the run).
fn within_tolerance(hits: u64, n: u64, p: f64) -> bool {
    let sigma = (p * (1.0 - p) / n as f64).sqrt();
    (hits as f64 / n as f64 - p).abs() <= 5.0 * sigma + 1e-12
}

#[test]
fn constraint_solver_matches_lrm_dist_and_solve_before_examples_on_wide_variables() {
    let mut rng = Rng(1);
    // 18.5.4: x dist { [0:999] :/ 600, [1000:1999] := 1, 5000 := 400 } with
    // x != 1500: the ranges carry 600, 999 and 400 of 1999.
    let mut g = Gen {
        rng: &mut rng,
        p: Problem::default(),
        allow_calls: false,
        wide: false,
        extended: false,
    };
    g.p.vars.push(new_var(16, false));
    let x = var_node(&mut g, 0);
    let a = dist_item(&mut g, x, 0, 999, Some(600), true);
    let b = dist_item(&mut g, x, 1000, 1999, Some(1), false);
    let c = dist_item(&mut g, x, 5000, 5000, Some(400), false);
    add_dist(&mut g, x, None, vec![a, b, c], true);
    let excluded = g.constant(16, false, 1500);
    let ne = g.push(NE, false, 1, x, excluded, 0);
    g.p.blocks.push((ne, true));
    let dist_problem = g.p;

    // 18.5.9: s -> d == 0 with solve s before d: s is 1 half the time; the
    // same problem without the order makes s == 1 a 1-in-65537 event.
    let solve_problem = |ordered: bool| {
        let mut rng = Rng(2);
        let mut g = Gen {
            rng: &mut rng,
            p: Problem::default(),
            allow_calls: false,
            wide: false,
            extended: false,
        };
        g.p.vars.push(new_var(1, false));
        g.p.vars.push(new_var(16, false));
        let s = var_node(&mut g, 0);
        let d = var_node(&mut g, 1);
        let zero = g.constant(16, false, 0);
        let eq = g.push(EQ, false, 1, d, zero, 0);
        let root = g.push(IMPL, false, 1, s, eq, 0);
        g.p.blocks.push((root, true));
        if ordered {
            g.p.orders.push((0, 0, 1));
        }
        g.p
    };
    // S30-D1: `if (g) y dist { 0 := 5, [1:15] :/ 5 }` reweights y only where
    // g holds: g stays uniform (the dist covers y's whole range, like
    // `inside`) and y == 0 half the time under g.
    let mut rng = Rng(5);
    let mut g = Gen {
        rng: &mut rng,
        p: Problem::default(),
        allow_calls: false,
        wide: false,
        extended: false,
    };
    g.p.vars.push(new_var(1, false));
    g.p.vars.push(new_var(4, false));
    let guard = var_node(&mut g, 0);
    let y = var_node(&mut g, 1);
    let zero = dist_item(&mut g, y, 0, 0, Some(5), false);
    let rest = dist_item(&mut g, y, 1, 15, Some(5), true);
    add_dist(&mut g, y, Some(guard), vec![zero, rest], true);
    let guarded = g.p;
    let problems = vec![
        dist_problem,
        solve_problem(true),
        solve_problem(false),
        guarded,
    ];
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-lrm-weights") else {
        return;
    };
    let n = 20_000u64;
    let output = run_probe(&executable, n as u32, &[]);
    let outcomes = parse(&output, problems.len());
    for (i, o) in outcomes.iter().enumerate() {
        assert_eq!(o.samples.len(), n as usize, "problem {i}: {}", o.detail);
    }
    let in_range = |lo: u128, hi: u128| {
        outcomes[0]
            .samples
            .iter()
            .filter(|s| (lo..=hi).contains(&s[0]))
            .count() as u64
    };
    assert_eq!(in_range(0, 1999) + in_range(5000, 5000), n);
    assert!(outcomes[0].samples.iter().all(|s| s[0] != 1500));
    assert!(within_tolerance(in_range(0, 999), n, 600.0 / 1999.0));
    assert!(within_tolerance(in_range(1000, 1999), n, 999.0 / 1999.0));
    assert!(within_tolerance(in_range(5000, 5000), n, 400.0 / 1999.0));
    // :/ spreads 600 evenly: the lower half of [0:999] carries half of it.
    assert!(within_tolerance(in_range(0, 499), n, 300.0 / 1999.0));
    let s_true = |o: &Outcome| o.samples.iter().filter(|s| s[0] == 1).count() as u64;
    assert!(within_tolerance(s_true(&outcomes[1]), n, 0.5));
    assert!(
        outcomes[1].samples.iter().all(|s| s[0] == 0 || s[1] == 0),
        "the order must not change the solution space"
    );
    assert!(
        s_true(&outcomes[2]) <= 3,
        "unordered s is 1 for one solution of 65537"
    );
    let guarded = &outcomes[3].samples;
    let g_true = guarded.iter().filter(|s| s[0] == 1).count() as u64;
    let zero_under_g = guarded.iter().filter(|s| s[0] == 1 && s[1] == 0).count() as u64;
    let zero_otherwise = guarded.iter().filter(|s| s[0] == 0 && s[1] == 0).count() as u64;
    assert!(within_tolerance(g_true, n, 0.5), "g={g_true}");
    assert!(
        within_tolerance(zero_under_g, g_true, 0.5),
        "{zero_under_g}/{g_true}"
    );
    assert!(
        within_tolerance(zero_otherwise, n - g_true, 1.0 / 16.0),
        "{zero_otherwise}/{}",
        n - g_true
    );
}

#[test]
fn constraint_solver_reports_circular_orders_and_negative_weights() {
    let mut rng = Rng(3);
    let mut g = Gen {
        rng: &mut rng,
        p: Problem::default(),
        allow_calls: false,
        wide: false,
        extended: false,
    };
    g.p.vars.push(new_var(2, false));
    g.p.vars.push(new_var(2, false));
    let a = var_node(&mut g, 0);
    let b = var_node(&mut g, 1);
    let lt = g.push(LT, false, 1, a, b, 0);
    g.p.blocks.push((lt, true));
    g.p.orders.push((0, 0, 1));
    g.p.orders.push((0, 1, 0));
    let circular = g.p.clone();

    let mut g = Gen {
        rng: &mut rng,
        p: Problem::default(),
        allow_calls: false,
        wide: false,
        extended: false,
    };
    g.p.vars.push(new_var(3, false));
    let x = var_node(&mut g, 0);
    let mut item = dist_item(&mut g, x, 1, 1, None, false);
    item.weight = Some(g.constant(4, true, 0xf));
    add_dist(&mut g, x, None, vec![item], true);
    let negative = g.p;

    // A circular order in a disabled block does not take part.
    let mut disabled = circular.clone();
    disabled.blocks.push((lt, false));
    disabled.orders = vec![(0, 0, 1), (1, 1, 0)];

    let problems = vec![circular, negative, disabled];
    let Some((_dir, executable)) = build_probe(&problems, "runtime-constraints-order-errors")
    else {
        return;
    };
    let outcomes = parse(&run_probe(&executable, 8, &[]), problems.len());
    // LLG_CR_ERROR with LLG_CR_REASON_SOLVE_BEFORE (9) and LLG_CR_REASON_WEIGHT (8).
    assert_eq!(outcomes[0].statuses, vec![3]);
    assert!(
        outcomes[0].detail.starts_with("r9 "),
        "{}",
        outcomes[0].detail
    );
    assert_eq!(outcomes[1].statuses, vec![3]);
    assert!(
        outcomes[1].detail.starts_with("r8 "),
        "{}",
        outcomes[1].detail
    );
    assert_eq!(outcomes[2].samples.len(), 8, "{}", outcomes[2].detail);
}

const RANDC_MAIN: &str = r#"
static uint32_t next_draw(void* stream) {
    uint64_t* x = (uint64_t*)stream;
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    return (uint32_t)(*x >> 32);
}

/* argv: samples, objects, phase. From sample `phase` on, the problem's last
 * block is disabled. Each object keeps its own randc cycle state, set before
 * and stored after every successful solve, as generated code does. */
int main(int argc, char** argv) {
    if (argc < 4) return 2;
    uint32_t samples = (uint32_t)atoi(argv[1]);
    uint32_t objects = (uint32_t)atoi(argv[2]);
    uint32_t phase = (uint32_t)atoi(argv[3]);
    static llg_cr_session_t* slots[PROBLEM_COUNT];
    for (uint32_t i = 0; i < PROBLEM_COUNT; ++i) {
        const llg_cr_problem_t* p = problems[i];
        uint64_t stream = UINT64_C(0x9e3779b97f4a7c15) ^ ((uint64_t)i * 7919u + 1u);
        llg_cr_rng_t rng = {next_draw, &stream};
        llg_cr_session_t* s = llg_cr_session_get(p, &slots[i]);
        size_t stride = LLG_CR_CYCLIC_WORDS(LLG_CR_CYCLIC_MAX_WIDTH);
        uint64_t* state = (uint64_t*)calloc((size_t)objects * p->var_count * stride, 8u);
        for (uint32_t k = 0; k < samples; ++k) {
            uint32_t object = k % objects;
            inputs[i](s);
            if (k >= phase) llg_cr_set_block(s, p->block_count - 1u, 0);
            for (uint32_t v = 0; v < p->var_count; ++v)
                if (p->vars[v].cyclic)
                    llg_cr_set_cyclic(s, v, state + ((size_t)object * p->var_count + v) * stride);
            llg_cr_report_t report;
            llg_cr_status_t status = llg_cr_solve(s, &rng, NULL, NULL, &report);
            printf("P%u %d", i, (int)status);
            if (status == LLG_CR_OK) {
                for (uint32_t v = 0; v < p->var_count; ++v) {
                    const uint64_t* w = llg_cr_value(s, v);
                    printf(" %016llx%016llx", 0ull, (unsigned long long)w[0]);
                    if (p->vars[v].cyclic) {
                        uint32_t n = LLG_CR_CYCLIC_WORDS(p->vars[v].bit_width);
                        const uint64_t* c = llg_cr_cyclic(s, v);
                        for (uint32_t j = 0; j < n; ++j)
                            state[((size_t)object * p->var_count + v) * stride + j] = c[j];
                    }
                }
                printf(" e%u", report.engines);
            } else if (status == LLG_CR_RESOURCE) {
                printf(" %s", report.budget);
            } else if (status == LLG_CR_ERROR) {
                printf(" r%d i%u", report.reason, report.item);
            }
            printf("\n");
            if (status != LLG_CR_OK) break;
        }
        free(state);
    }
    llg_cr_session_release_all();
    return 0;
}
"#;

fn run_randc(
    executable: &std::path::Path,
    samples: u32,
    objects: u32,
    phase: u32,
    env: &[(&str, &str)],
) -> String {
    let mut command = Command::new(executable);
    command.args([samples.to_string(), objects.to_string(), phase.to_string()]);
    for name in [
        "LLG_RAND_BDD_NODES",
        "LLG_RAND_BDD_BITS",
        "LLG_RAND_REJECT_TRIES",
        "LLG_RAND_CONFLICTS",
        "LLG_RAND_STATS",
    ] {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let output = sim_harness::run_command(&mut command, Duration::from_secs(300))
        .unwrap_or_else(|error| panic!("run randc probe: {error}"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Check one object's sequence of a randc variable against the S30-D3
/// model: a value repeats only after a new cycle, which starts when the
/// constraints change (`phase`) or when no unused value is feasible.
fn check_cycles(values: &[(u32, u128)], feasible: &dyn Fn(u32) -> BTreeSet<u128>, phase: u32) {
    let mut used = BTreeSet::new();
    let mut last_phase = false;
    for &(k, value) in values {
        let in_phase = k >= phase;
        if in_phase != last_phase {
            used.clear();
            last_phase = in_phase;
        }
        let allowed = feasible(k);
        assert!(
            allowed.contains(&value),
            "sample {k}: {value} is infeasible"
        );
        if allowed.iter().all(|v| used.contains(v)) {
            used.clear();
        }
        assert!(
            used.insert(value),
            "sample {k}: {value} repeats within a cycle ({used:?})"
        );
    }
}

#[test]
fn randc_cycles_per_object_without_repeats_and_restarts_on_constraint_changes() {
    let mut rng = Rng(4);
    fn gen(rng: &mut Rng) -> Gen<'_> {
        Gen {
            rng,
            p: Problem::default(),
            allow_calls: false,
            wide: false,
            extended: false,
        }
    }
    // P0: randc x (3 bits): x != 5, and x < 6 until the phase.
    let mut g = gen(&mut rng);
    g.p.vars.push(Var {
        cyclic: true,
        ..new_var(3, false)
    });
    let x = var_node(&mut g, 0);
    let five = g.constant(3, false, 5);
    let six = g.constant(3, false, 6);
    let ne = g.push(NE, false, 1, x, five, 0);
    let lt = g.push(LT, false, 1, x, six, 0);
    g.p.blocks.push((ne, true));
    g.p.blocks.push((lt, true));
    let p0 = g.p;
    // P1: unconstrained randc y (4 bits): the direct path. Its last block
    // constrains another variable, so the phase does not touch y.
    let mut g = gen(&mut rng);
    g.p.vars.push(Var {
        cyclic: true,
        ..new_var(4, false)
    });
    g.p.vars.push(new_var(2, false));
    let z = var_node(&mut g, 1);
    let one = g.constant(2, false, 1);
    let zne = g.push(NE, false, 1, z, one, 0);
    g.p.blocks.push((zne, true));
    let p1 = g.p;
    // P2: randc a (2 bits) solved before rand b (3 bits) with b > a: a
    // cycles; b is uniform given a.
    let mut g = gen(&mut rng);
    g.p.vars.push(Var {
        cyclic: true,
        ..new_var(2, false)
    });
    g.p.vars.push(new_var(3, false));
    let a = var_node(&mut g, 0);
    let b = var_node(&mut g, 1);
    let a3 = g.resize(a, 3, false);
    let gt = g.push(GT, false, 1, b, a3, 0);
    g.p.blocks.push((gt, true));
    // The phase disables a constant block: no structural change.
    let t = g.constant(1, false, 1);
    g.p.blocks.push((t, true));
    let p2 = g.p;
    // P3: two randc variables tied by u + v == 3 (2 bits each): v's cycle
    // must restart whenever u leaves it no unused value.
    let mut g = gen(&mut rng);
    for _ in 0..2 {
        g.p.vars.push(Var {
            cyclic: true,
            ..new_var(2, false)
        });
    }
    let u = var_node(&mut g, 0);
    let v = var_node(&mut g, 1);
    let sum = g.push(ADD, false, 2, u, v, 0);
    let three = g.constant(2, false, 3);
    let eq = g.push(EQ, false, 1, sum, three, 0);
    g.p.blocks.push((eq, true));
    let t = g.constant(1, false, 1);
    g.p.blocks.push((t, true));
    let p3 = g.p;

    let problems = vec![p0, p1, p2, p3];
    let Some((_dir, executable)) =
        build_probe_main(&problems, "runtime-constraints-randc", RANDC_MAIN)
    else {
        return;
    };
    let (samples, objects, phase) = (4000u32, 2u32, 20u32);
    let outcomes = parse(
        &run_randc(&executable, samples, objects, phase, &[]),
        problems.len(),
    );
    for (i, o) in outcomes.iter().enumerate() {
        assert_eq!(
            o.samples.len(),
            samples as usize,
            "problem {i}: {}",
            o.detail
        );
    }
    for object in 0..objects {
        let of = |i: usize, var: usize| -> Vec<(u32, u128)> {
            outcomes[i]
                .samples
                .iter()
                .enumerate()
                .filter(|(k, _)| *k as u32 % objects == object)
                .map(|(k, s)| (k as u32, s[var]))
                .collect()
        };
        let p0_feasible = |k: u32| -> BTreeSet<u128> {
            (0..8)
                .filter(|&x| x != 5 && (k >= phase || x < 6))
                .collect()
        };
        check_cycles(&of(0, 0), &p0_feasible, phase);
        check_cycles(&of(1, 0), &|_| (0..16).collect(), u32::MAX);
        check_cycles(&of(2, 0), &|_| (0..4).collect(), u32::MAX);
        check_cycles(&of(3, 0), &|_| (0..4).collect(), u32::MAX);
        // v = 3 - u is forced: its only feasible value is 3 - u, and its
        // cycle restarts when that value was already used.
        let us: BTreeMap<u32, u128> = of(3, 0).into_iter().collect();
        check_cycles(&of(3, 1), &|k| BTreeSet::from([3 - us[&k]]), u32::MAX);
    }
    // Before the phase, object 0's first cycle of P0 is a permutation of
    // its five feasible values: cycles are per object.
    let first: BTreeSet<u128> = outcomes[0]
        .samples
        .iter()
        .step_by(objects as usize)
        .take(5)
        .map(|s| s[0])
        .collect();
    assert_eq!(first, (0..5).collect());
    // Uniform cycle starts: the first value of each 16-cycle of P1.
    let mut starts: BTreeMap<Vec<u128>, u64> = BTreeMap::new();
    let ys: Vec<u128> = outcomes[1]
        .samples
        .iter()
        .step_by(objects as usize)
        .map(|s| s[0])
        .collect();
    for chunk in ys.as_chunks::<16>().0 {
        let set: BTreeSet<u128> = chunk.iter().copied().collect();
        assert_eq!(set.len(), 16, "a 16-cycle is a permutation");
        *starts.entry(vec![chunk[0]]).or_default() += 1;
    }
    assert!(
        chi_square_ok(&starts, 16, (ys.len() / 16) as u64),
        "{starts:?}"
    );
    // P2: b uniform over (a, 7] given a.
    let mut pairs: BTreeMap<Vec<u128>, u64> = BTreeMap::new();
    for s in &outcomes[2].samples {
        assert!(s[1] > s[0]);
        *pairs.entry(vec![s[0], s[1]]).or_default() += 1;
    }
    let mut expectation = BTreeMap::new();
    for a in 0..4u128 {
        for b in a + 1..8 {
            expectation.insert(vec![a, b], 0.25 / (7 - a) as f64);
        }
    }
    let (ok, chi) = weighted_chi_square_ok(&pairs, &expectation, samples as u64);
    assert!(ok, "chi-square {chi:.1}: {pairs:?}");

    // Without the exact engine a constrained randc variable is a resource
    // error (S30-D9); an unconstrained one still cycles.
    let outcomes = parse(
        &run_randc(&executable, 64, 1, u32::MAX, &[("LLG_RAND_BDD_NODES", "1")]),
        problems.len(),
    );
    assert_eq!(outcomes[0].statuses, vec![2], "{}", outcomes[0].detail);
    assert_eq!(outcomes[1].samples.len(), 64, "{}", outcomes[1].detail);
}
