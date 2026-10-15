//! SIM-029: constrained randomization (IEEE 1800-2009 clause 18). Positive
//! fixtures print relations that hold for every legal solution, so their
//! expected output does not depend on the values drawn; the readme quotes the
//! clauses each line checks. Negative fixtures name every construct outside
//! the SIM-029 scope: none may be silently left unconstrained.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_029";

/// Budgets that leave the solver no work: every satisfiable problem that
/// needs a search ends with RESOURCE.
const NO_BUDGET: [(&str, &str); 3] = [
    ("LLG_RAND_BDD_NODES", "1"),
    ("LLG_RAND_REJECT_TRIES", "0"),
    ("LLG_RAND_CONFLICTS", "0"),
];

#[test]
fn class_constraints_inheritance_and_static_members() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_constraints",
        include_str!("../fixtures/sim/feature_completion/sim_029/class_constraints.out"),
        &[],
        &[],
    );
}

#[test]
fn rand_mode_and_constraint_mode() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "modes",
        include_str!("../fixtures/sim/feature_completion/sim_029/modes.out"),
        &[],
        &[],
    );
}

#[test]
fn failed_randomize_changes_nothing_and_hooks_run_in_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unsat_and_hooks",
        include_str!("../fixtures/sim/feature_completion/sim_029/unsat_and_hooks.out"),
        &[],
        &[],
    );
}

#[test]
fn inline_constraints_variable_lists_and_scope_randomize() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "inline_and_lists",
        include_str!("../fixtures/sim/feature_completion/sim_029/inline_and_lists.out"),
        &[],
        &[],
    );
}

#[test]
fn signed_ranges_are_exact() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "signed_ranges",
        include_str!("../fixtures/sim/feature_completion/sim_029/signed_ranges.out"),
        &[],
        &[],
    );
}

#[test]
fn constraint_functions_and_guards() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "functions_and_guards",
        include_str!("../fixtures/sim/feature_completion/sim_029/functions_and_guards.out"),
        &[],
        &[],
    );
}

/// Multiplications beyond the exact engine's budget and too rare for
/// rejection: the CDCL fallback finds solutions and proves infeasibility.
#[test]
fn search_fallback_solves_and_proves_unsat() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "search_fallback",
        include_str!("../fixtures/sim/feature_completion/sim_029/search_fallback.out"),
        &[],
        &[],
    );
}

/// A located rejection names the fixture's source position, not only the
/// construct.
fn names_source_position(fixture: &str) {
    let source = format!("{fixture}.sv:");
    let output = sim_cli::invoke_with_env(SUITE, fixture, false, &[], &[], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&source),
        "{fixture}: the diagnostic names the source position: {stderr}"
    );
}

/// SIM-030 implements `randc`, `dist`, `solve...before`, random divisors,
/// random select indices and `**` (see `sim_030.rs`); `foreach` constraints
/// remain a located error until its array support lands.
#[test]
fn sim_030_constructs_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_foreach",
        "`foreach` constraints are not supported (planned for SIM-030) at ",
    );
    names_source_position("neg_foreach");
}

#[test]
fn unsupported_random_variables_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_rand_array",
        "random property `v` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_rand_array");
    sim_cli::reject_case(
        SUITE,
        "neg_rand_handle",
        "random property `l` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_rand_handle");
    sim_cli::reject_case(
        SUITE,
        "neg_enum_struct",
        "random property `s` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_enum_struct");
}

#[test]
fn value_randomize_outside_one_evaluation_is_a_located_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_short_circuit",
        "a randomize call in a short-circuit or conditional operand is not supported",
    );
    names_source_position("neg_short_circuit");
    sim_cli::reject_case(
        SUITE,
        "neg_loop_condition",
        "a randomize call used as a value is supported only in",
    );
    names_source_position("neg_loop_condition");
}

/// A run-time failure: exact stdout, exit status 1 and `diagnostic` on
/// stderr.
fn runtime_failure<'a>(stdout: &'a str, diagnostic: &'a str) -> impl Fn(&str, &Output) + 'a {
    move |label: &str, output: &Output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        assert!(stderr.contains(diagnostic), "{label}: {stderr}");
    }
}

#[test]
fn null_receiver_is_a_runtime_error() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_null_receiver",
        &[],
        &runtime_failure(
            "before\n",
            "null class handle access: randomize() at tb:8:10",
        ),
    );
}

#[test]
fn unguarded_null_reference_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_unguarded_null",
        &[],
        &runtime_failure(
            "ok=0 v=5\n",
            "llg: error: randomize() at tb:14:10: null object handle in a constraint",
        ),
    );
}

#[test]
fn circular_function_ordering_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_circular_order",
        &[],
        &runtime_failure(
            "ok=0\n",
            "llg: error: randomize() at tb:12:10: circular function-argument order (`x`)",
        ),
    );
}

/// A03: an exhausted budget is a run-time error naming the budget, distinct
/// from the warning of an unsatisfiable problem.
#[test]
fn exhausted_budget_is_distinct_from_unsat() {
    let failure = runtime_failure(
        "unsat ok=0\nresource ok=0 x=0 y=0\n",
        "llg: error: randomize() at tb:18:10: solver budget",
    );
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "rt_resource",
        &[],
        &NO_BUDGET,
        &|label: &str, output: &Output| {
            failure(label, output);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(
                    "llg: warning: randomize() at tb:16:10: the constraints cannot be satisfied; randomize returns 0"
                ),
                "{label}: {stderr}"
            );
        },
    );
}

/// A01 at the HDL level: a seeded generator writes small class problems
/// (2- to 4-bit signed or unsigned variables, a state property, an optional
/// variable switched off by `rand_mode(0)`, comparisons, arithmetic, `&&`,
/// `||`, `!`, `->`, `if`/`else` and `inside`), the simulator randomizes each
/// one repeatedly, and an independent Rust enumerator of every assignment
/// checks the outcome: UNSAT exactly when no assignment satisfies the
/// constraints (and then every variable keeps its value, 18.6.3), every
/// sample a solution, inactive variables unchanged (18.8), and samples
/// uniform over small solution sets (18.5.9, chi-square).
mod generated {
    use super::super::sim_harness;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Write as _;

    /// SplitMix64, so the problem set is identical on every host.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    enum Arith {
        Var(usize),
        State,
        Const(u64),
        Bin(&'static str, Box<Arith>, Box<Arith>),
    }

    enum Bool {
        Cmp(&'static str, Arith, Arith),
        And(Box<Bool>, Box<Bool>),
        Or(Box<Bool>, Box<Bool>),
        Not(Box<Bool>),
        Inside(Arith, Vec<(u64, u64)>),
    }

    enum Item {
        Expr(Bool),
        Implies(Bool, Bool),
        IfElse(Bool, Bool, Bool),
    }

    struct Problem {
        width: u32,
        signed: bool,
        vars: usize,
        state: u64,
        /// The last variable is inactive (`rand_mode(0)`) when set.
        frozen: bool,
        initial: Vec<u64>,
        items: Vec<Item>,
    }

    const NAMES: [&str; 3] = ["a", "b", "c"];
    const SAMPLES: usize = 160;

    impl Problem {
        fn mask(&self) -> u64 {
            (1u64 << self.width) - 1
        }

        /// The value as the comparison sees it: sign-extended when signed.
        fn ordered(&self, value: u64) -> i64 {
            let value = value & self.mask();
            if self.signed && value >> (self.width - 1) & 1 == 1 {
                value as i64 - (1i64 << self.width)
            } else {
                value as i64
            }
        }

        fn constant(&self, value: u64) -> String {
            let sign = if self.signed { "s" } else { "" };
            format!("{}'{sign}h{value:x}", self.width)
        }

        fn arith_sv(&self, a: &Arith) -> String {
            match a {
                Arith::Var(v) => NAMES[*v].to_owned(),
                Arith::State => "s".to_owned(),
                Arith::Const(c) => self.constant(*c),
                Arith::Bin(op, l, r) => {
                    format!("({} {op} {})", self.arith_sv(l), self.arith_sv(r))
                }
            }
        }

        fn bool_sv(&self, b: &Bool) -> String {
            match b {
                Bool::Cmp(op, l, r) => {
                    format!("({} {op} {})", self.arith_sv(l), self.arith_sv(r))
                }
                Bool::And(l, r) => format!("({} && {})", self.bool_sv(l), self.bool_sv(r)),
                Bool::Or(l, r) => format!("({} || {})", self.bool_sv(l), self.bool_sv(r)),
                Bool::Not(x) => format!("(!{})", self.bool_sv(x)),
                Bool::Inside(x, ranges) => {
                    let ranges: Vec<String> = ranges
                        .iter()
                        .map(|(lo, hi)| {
                            if lo == hi {
                                self.constant(*lo)
                            } else {
                                format!("[{}:{}]", self.constant(*lo), self.constant(*hi))
                            }
                        })
                        .collect();
                    format!("({} inside {{{}}})", self.arith_sv(x), ranges.join(", "))
                }
            }
        }

        fn arith(&self, a: &Arith, values: &[u64]) -> u64 {
            let m = self.mask();
            match a {
                Arith::Var(v) => values[*v],
                Arith::State => self.state,
                Arith::Const(c) => *c,
                Arith::Bin(op, l, r) => {
                    let (l, r) = (self.arith(l, values), self.arith(r, values));
                    (match *op {
                        "+" => l.wrapping_add(r),
                        "-" => l.wrapping_sub(r),
                        "*" => l.wrapping_mul(r),
                        "&" => l & r,
                        "|" => l | r,
                        _ => l ^ r,
                    }) & m
                }
            }
        }

        fn holds(&self, b: &Bool, values: &[u64]) -> bool {
            match b {
                Bool::Cmp(op, l, r) => {
                    let l = self.ordered(self.arith(l, values));
                    let r = self.ordered(self.arith(r, values));
                    match *op {
                        "==" => l == r,
                        "!=" => l != r,
                        "<" => l < r,
                        "<=" => l <= r,
                        ">" => l > r,
                        _ => l >= r,
                    }
                }
                Bool::And(l, r) => self.holds(l, values) && self.holds(r, values),
                Bool::Or(l, r) => self.holds(l, values) || self.holds(r, values),
                Bool::Not(x) => !self.holds(x, values),
                Bool::Inside(x, ranges) => {
                    let x = self.ordered(self.arith(x, values));
                    ranges
                        .iter()
                        .any(|(lo, hi)| self.ordered(*lo) <= x && x <= self.ordered(*hi))
                }
            }
        }

        fn satisfied(&self, values: &[u64]) -> bool {
            self.items.iter().all(|item| match item {
                Item::Expr(b) => self.holds(b, values),
                Item::Implies(p, q) => !self.holds(p, values) || self.holds(q, values),
                Item::IfElse(p, q, r) => {
                    if self.holds(p, values) {
                        self.holds(q, values)
                    } else {
                        self.holds(r, values)
                    }
                }
            })
        }

        fn active(&self) -> usize {
            self.vars - usize::from(self.frozen)
        }

        /// Every assignment of the active variables that satisfies the
        /// constraints, with inactive variables at their initial values.
        fn solutions(&self) -> BTreeSet<Vec<u64>> {
            let mut out = BTreeSet::new();
            let span = 1u64 << self.width;
            let active = self.active();
            for code in 0..span.pow(active as u32) {
                let mut values = self.initial.clone();
                for (v, value) in values.iter_mut().enumerate().take(active) {
                    *value = code / span.pow(v as u32) % span;
                }
                if self.satisfied(&values) {
                    out.insert(values);
                }
            }
            out
        }
    }

    fn arith(rng: &mut Rng, p: &Problem, depth: u32) -> Arith {
        match rng.below(if depth == 0 { 3 } else { 5 }) {
            0 | 1 => Arith::Var(rng.below(p.vars as u64) as usize),
            2 if rng.below(3) == 0 => Arith::State,
            2 => Arith::Const(rng.next() & p.mask()),
            _ => {
                let op = ["+", "-", "*", "&", "|", "^"][rng.below(6) as usize];
                Arith::Bin(
                    op,
                    Box::new(arith(rng, p, depth - 1)),
                    Box::new(arith(rng, p, depth - 1)),
                )
            }
        }
    }

    fn range(rng: &mut Rng, p: &Problem) -> (u64, u64) {
        let (x, y) = (rng.next() & p.mask(), rng.next() & p.mask());
        if p.ordered(x) <= p.ordered(y) {
            (x, y)
        } else {
            (y, x)
        }
    }

    fn boolean(rng: &mut Rng, p: &Problem, depth: u32) -> Bool {
        match rng.below(if depth == 0 { 2 } else { 6 }) {
            0 => {
                let op = ["==", "!=", "<", "<=", ">", ">="][rng.below(6) as usize];
                Bool::Cmp(op, arith(rng, p, 1), arith(rng, p, 1))
            }
            1 => {
                let count = 1 + rng.below(3);
                let ranges = (0..count)
                    .map(|_| {
                        if rng.below(3) == 0 {
                            let v = rng.next() & p.mask();
                            (v, v)
                        } else {
                            range(rng, p)
                        }
                    })
                    .collect();
                Bool::Inside(arith(rng, p, 1), ranges)
            }
            2 => Bool::And(
                Box::new(boolean(rng, p, depth - 1)),
                Box::new(boolean(rng, p, depth - 1)),
            ),
            3 => Bool::Or(
                Box::new(boolean(rng, p, depth - 1)),
                Box::new(boolean(rng, p, depth - 1)),
            ),
            4 => Bool::Not(Box::new(boolean(rng, p, depth - 1))),
            _ => boolean(rng, p, 0),
        }
    }

    fn generate(seed: u64) -> Problem {
        let mut rng = Rng(seed);
        let width = 2 + rng.below(3) as u32;
        let vars = 1 + rng.below(3) as usize;
        let mut p = Problem {
            width,
            signed: rng.below(2) == 1,
            vars,
            state: 0,
            frozen: vars > 1 && rng.below(4) == 0,
            initial: Vec::new(),
            items: Vec::new(),
        };
        p.state = rng.next() & p.mask();
        p.initial = (0..vars).map(|_| rng.next() & p.mask()).collect();
        for _ in 0..1 + rng.below(3) {
            let item = match rng.below(4) {
                0 => Item::Implies(boolean(&mut rng, &p, 1), boolean(&mut rng, &p, 1)),
                1 => Item::IfElse(
                    boolean(&mut rng, &p, 1),
                    boolean(&mut rng, &p, 1),
                    boolean(&mut rng, &p, 1),
                ),
                _ => Item::Expr(boolean(&mut rng, &p, 2)),
            };
            p.items.push(item);
        }
        p
    }

    fn source(problems: &[Problem]) -> String {
        let mut sv = String::new();
        for (k, p) in problems.iter().enumerate() {
            let sign = if p.signed { " signed" } else { "" };
            let msb = p.width - 1;
            let names = NAMES[..p.vars].join(", ");
            writeln!(sv, "class p{k}_c;").unwrap();
            writeln!(sv, "  rand bit{sign} [{msb}:0] {names};").unwrap();
            writeln!(sv, "  bit{sign} [{msb}:0] s;").unwrap();
            writeln!(sv, "  constraint cons {{").unwrap();
            for item in &p.items {
                let text = match item {
                    Item::Expr(b) => format!("{};", p.bool_sv(b)),
                    Item::Implies(q, r) => format!("{} -> {};", p.bool_sv(q), p.bool_sv(r)),
                    Item::IfElse(q, r, t) => format!(
                        "if ({}) {{ {}; }} else {{ {}; }}",
                        p.bool_sv(q),
                        p.bool_sv(r),
                        p.bool_sv(t)
                    ),
                };
                writeln!(sv, "    {text}").unwrap();
            }
            writeln!(sv, "  }}\nendclass").unwrap();
        }
        writeln!(sv, "module tb;\n  initial begin\n    int ok;").unwrap();
        for (k, p) in problems.iter().enumerate() {
            writeln!(sv, "    begin\n      static p{k}_c o = new;").unwrap();
            writeln!(sv, "      o.s = {};", p.constant(p.state)).unwrap();
            for (name, value) in NAMES.iter().zip(&p.initial) {
                writeln!(sv, "      o.{name} = {};", p.constant(*value)).unwrap();
            }
            if p.frozen {
                writeln!(sv, "      o.{}.rand_mode(0);", NAMES[p.vars - 1]).unwrap();
            }
            let values: Vec<String> = NAMES[..p.vars].iter().map(|n| format!("o.{n}")).collect();
            let formats = vec![" %0h"; p.vars].concat();
            writeln!(
                sv,
                "      for (int i = 0; i < {SAMPLES}; i++) begin\n        ok = o.randomize();\n        \
                 $display(\"P{k} %0d{formats}\", ok, {});\n        if (ok == 0) break;\n      end\n    end",
                values.join(", ")
            )
            .unwrap();
        }
        writeln!(sv, "    $finish;\n  end\nendmodule").unwrap();
        sv
    }

    /// Generous chi-square bound (df = k - 1): a correct sampler with a
    /// fixed seed stays far below it, a per-branch or per-range bias does not.
    fn uniform(counts: &BTreeMap<Vec<u64>, u64>, k: usize, n: u64) -> bool {
        let expected = n as f64 / k as f64;
        let mut chi: f64 = counts
            .values()
            .map(|&c| (c as f64 - expected).powi(2) / expected)
            .sum();
        chi += (k - counts.len()) as f64 * expected;
        let df = (k - 1) as f64;
        chi <= df + 6.0 * (2.0 * df).sqrt() + 10.0
    }

    /// Checks one run's output; returns (sat, unsat, uniformity-checked).
    fn check(problems: &[Problem], stdout: &str) -> (usize, usize, usize) {
        let mut lines: BTreeMap<usize, Vec<(u64, Vec<u64>)>> = BTreeMap::new();
        for line in stdout.lines().filter(|l| l.starts_with('P')) {
            let mut fields = line.split_whitespace();
            let k: usize = fields.next().unwrap()[1..].parse().unwrap();
            let ok: u64 = fields.next().unwrap().parse().unwrap();
            let values = fields
                .map(|f| u64::from_str_radix(f, 16).unwrap())
                .collect();
            lines.entry(k).or_default().push((ok, values));
        }
        let (mut sat, mut unsat, mut uniform_checked) = (0, 0, 0);
        for (k, p) in problems.iter().enumerate() {
            let runs = lines
                .get(&k)
                .unwrap_or_else(|| panic!("problem {k}: no output"));
            let solutions = p.solutions();
            if solutions.is_empty() {
                unsat += 1;
                assert_eq!(runs.len(), 1, "problem {k}: one failed call");
                assert_eq!(
                    runs[0],
                    (0, p.initial.clone()),
                    "problem {k}: UNSAT changes nothing"
                );
                continue;
            }
            sat += 1;
            assert_eq!(runs.len(), SAMPLES, "problem {k}: every call succeeds");
            let mut counts: BTreeMap<Vec<u64>, u64> = BTreeMap::new();
            for (ok, values) in runs {
                assert_eq!(*ok, 1, "problem {k}");
                assert!(
                    solutions.contains(values),
                    "problem {k}: {values:?} is not a solution"
                );
                *counts.entry(values.clone()).or_default() += 1;
            }
            if (2..=16).contains(&solutions.len()) {
                uniform_checked += 1;
                assert!(
                    uniform(&counts, solutions.len(), SAMPLES as u64),
                    "problem {k}: not uniform over {} solutions: {counts:?}",
                    solutions.len()
                );
            }
        }
        (sat, unsat, uniform_checked)
    }

    #[test]
    fn generated_problems_match_an_independent_enumerator() {
        let problems: Vec<Problem> = (0..48u64)
            .map(|seed| generate(0x5129_0000 + seed))
            .collect();
        let sv = source(&problems);
        let mut outputs = Vec::new();
        for (label, options) in [
            ("unoptimized", llg::sim::opt::OptConfig::none()),
            ("optimized", llg::sim::opt::OptConfig::default()),
        ] {
            let run = sim_harness::run_generated_sim_with_files_opts(
                &sv,
                "tb",
                &format!("sim029-generated-{label}"),
                &[],
                &options,
            )
            .unwrap_or_else(|error| panic!("{label}: {error}\n{sv}"));
            let (sat, unsat, uniform_checked) = check(&problems, &run.stdout);
            eprintln!("{label}: sat={sat} unsat={unsat} uniform-checked={uniform_checked}");
            assert!(
                sat >= 24 && unsat >= 3 && uniform_checked >= 8,
                "{label}: the generator must exercise every outcome"
            );
            outputs.push(run.stdout);
        }
        // The same seed draws the same values whatever the optimizer did.
        assert_eq!(outputs[0], outputs[1]);
    }
}
