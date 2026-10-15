//! Foreseen exact-engine overflows (`cr_overflow_foreseen` in
//! `constraints/solve.c`) and cached search encodings (`cr_search_cached`).
//!
//! A component whose exact model overflowed `LLG_RAND_BDD_NODES` is proven
//! to overflow for every later state when the state-free part of its cone
//! alone exceeds the node bound; later builds of that shape whose root needs
//! the same state-free bits are skipped. The shortcut must never change a
//! drawn value: a skipped build is one that would have overflowed and fallen
//! back with the same draws, and a build that would succeed (here: the state
//! selects the other branch of a `?:`, a variable is inactive, a block is
//! disabled) still runs the exact engine. The search fallback reuses an
//! encoding only for the same cone and known values; a repeated state reuses
//! it, a changed state, `rand_mode` or `constraint_mode` does not. The probe
//! randomizes one site under a scripted sequence of states with both
//! shortcuts on and off (`LLG_RAND_OVERFLOW_MEMO=0`, `LLG_RAND_SEARCH_CACHE=0`)
//! and requires identical output.

use super::*;

const OVERFLOW_MAIN: &str = r#"
#include "llg_constraint.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t rng_next(void* state) {
    uint64_t* s = (uint64_t*)state;
    uint64_t x = *s;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    *s = x;
    return (uint32_t)(x >> 32);
}

#define W 10u
static llg_cr_node_t nodes[32];
static uint32_t nn;
static uint64_t consts[8];
static uint32_t nk;
static uint32_t args[8];
static uint32_t na;

static uint32_t nd(uint16_t op, uint32_t w, uint32_t a, uint32_t b, uint32_t c) {
    llg_cr_node_t* n = &nodes[nn];
    n->op = op; n->sign_flag = 0; n->reserved = 0; n->bit_width = w;
    n->a = a; n->b = b; n->c = c;
    return nn++;
}
static uint32_t k(uint64_t v, uint32_t w) {
    consts[nk] = v;
    return nd(LLG_CR_CONST, w, nk++, 0, 0);
}

static const llg_cr_var_t vars[2] = {
    {"x", W, 0, LLG_CR_NONE, 0},
    {"y", W, 0, LLG_CR_NONE, 0},
};
static const llg_cr_leaf_t leaves[2] = {{"t", 2u * W, 0}, {"g", 1u, 0}};
static llg_cr_block_t blocks[1];

int main(void) {
    /* block c { (g ? 20'(x) * 20'(y) : 20'(x) + 20'(y)) == t; x > 1; y > 1; } */
    uint32_t x = nd(LLG_CR_VAR, W, 0, 0, 0);
    uint32_t y = nd(LLG_CR_VAR, W, 1, 0, 0);
    uint32_t t = nd(LLG_CR_STATE, 2u * W, 0, 0, 0);
    uint32_t g = nd(LLG_CR_STATE, 1u, 1, 0, 0);
    uint32_t xw = nd(LLG_CR_RESIZE, 2u * W, x, 0, 0);
    uint32_t yw = nd(LLG_CR_RESIZE, 2u * W, y, 0, 0);
    uint32_t mul = nd(LLG_CR_MUL, 2u * W, xw, yw, 0);
    uint32_t add = nd(LLG_CR_ADD, 2u * W, xw, yw, 0);
    uint32_t sel = nd(LLG_CR_COND, 2u * W, g, mul, add);
    uint32_t eq = nd(LLG_CR_EQ, 1u, sel, t, 0);
    uint32_t one = k(1u, W);
    uint32_t gx = nd(LLG_CR_GT, 1u, x, one, 0);
    uint32_t gy = nd(LLG_CR_GT, 1u, y, one, 0);
    args[na++] = eq; args[na++] = gx; args[na++] = gy;
    uint32_t root = nd(LLG_CR_ALL, 1u, 0, 0, 3);
    blocks[0].name = "c";
    blocks[0].root = root;
    llg_cr_problem_t p;
    memset(&p, 0, sizeof(p));
    p.name = "overflow";
    p.nodes = nodes; p.node_count = nn;
    p.args = args; p.arg_count = na;
    p.consts = consts; p.const_count = nk;
    p.vars = vars; p.var_count = 2;
    p.blocks = blocks; p.block_count = 1;
    p.leaves = leaves; p.leaf_count = 2;
    llg_cr_session_t* s = llg_cr_session_get(&p, NULL);

    /* Primes in [503, 1021]: a product of two has exactly two factorings
     * with both factors above 1 and below 2^W. */
    uint32_t primes[100], np = 0;
    for (uint32_t c = 503; c < 1024u && np < 100u; c += 2u) {
        int prime = 1;
        for (uint32_t d = 3; d * d <= c; d += 2u) if (c % d == 0u) { prime = 0; break; }
        if (prime) primes[np++] = c;
    }
    uint64_t seed = UINT64_C(0x9E3779B97F4A7C15);
    llg_cr_rng_t rng = {rng_next, &seed};
    static const char* names[] = {"ok", "unsat", "resource", "error", "calls"};
    uint64_t xv = 3, yv = 5;
    for (uint32_t i = 0; i < 48u; ++i) {
        uint64_t gv = 1, tv, x_active = 1, block_on = 1;
        if (i < 8u) {
            tv = (uint64_t)primes[i % 4u] * primes[i % 4u + 1u];      /* repeats */
        } else if (i < 24u) {
            if (i % 2u) { gv = 0; tv = 100u + i % 3u; }               /* exact */
            else tv = (uint64_t)primes[i] * primes[i + 1u];          /* new */
        } else if (i < 28u) {
            x_active = 0; xv = 7; tv = 7u * (uint64_t)primes[i];      /* rand_mode(0) */
        } else if (i < 30u) {
            block_on = 0; tv = 0;                                     /* constraint_mode(0) */
        } else if (i < 40u) {
            tv = (uint64_t)primes[i + 10u] * primes[i + 11u];        /* new */
        } else if (i % 2u) {
            x_active = 0; xv = 7; tv = 7u * (uint64_t)primes[24];     /* repeats */
        } else {
            tv = (uint64_t)primes[i % 4u] * primes[i % 4u + 1u];      /* repeats */
        }
        llg_cr_begin(s);
        llg_cr_set_var(s, 0, &xv, (int)x_active, LLG_CR_INPUT_KNOWN);
        llg_cr_set_var(s, 1, &yv, 1, LLG_CR_INPUT_KNOWN);
        llg_cr_set_block(s, 0, (int)block_on);
        llg_cr_set_leaf(s, 0, &tv, LLG_CR_INPUT_KNOWN);
        llg_cr_set_leaf(s, 1, &gv, LLG_CR_INPUT_KNOWN);
        llg_cr_report_t report;
        llg_cr_status_t status = llg_cr_step(s, &rng, &report);
        if (status == LLG_CR_OK) {
            if (x_active) xv = llg_cr_value(s, 0)[0];
            yv = llg_cr_value(s, 1)[0];
        }
        printf("CALL %u g=%llu t=%llu x_active=%llu block=%llu status=%s engines=%u x=%llu y=%llu\n",
               i, (unsigned long long)gv, (unsigned long long)tv, (unsigned long long)x_active,
               (unsigned long long)block_on, names[status], report.engines,
               (unsigned long long)xv, (unsigned long long)yv);
    }
    llg_cr_session_release_all();
    return 0;
}
"#;

struct Call {
    g: u64,
    t: u64,
    x_active: bool,
    block: bool,
    status: String,
    engines: u32,
    x: u64,
    y: u64,
}

fn parse_calls(output: &str) -> Vec<Call> {
    output
        .lines()
        .filter(|line| line.starts_with("CALL "))
        .map(|line| {
            let field = |tag: &str| -> &str {
                line.split_whitespace()
                    .find_map(|word| word.strip_prefix(tag))
                    .unwrap_or_else(|| panic!("`{tag}` in `{line}`"))
            };
            let number = |tag: &str| -> u64 { field(tag).parse().expect("number") };
            Call {
                g: number("g="),
                t: number("t="),
                x_active: number("x_active=") == 1,
                block: number("block=") == 1,
                status: field("status=").to_owned(),
                engines: number("engines=") as u32,
                x: number("x="),
                y: number("y="),
            }
        })
        .collect()
}

/// Run the probe; returns stdout and the numbers of stats lines that report
/// a foreseen (skipped) exact build and a reused search encoding.
fn run_overflow_probe(exe: &std::path::Path, memo: &str, cache: &str) -> (String, usize, usize) {
    let mut command = Command::new(exe);
    for name in [
        "LLG_RAND_BDD_BITS",
        "LLG_RAND_REJECT_TRIES",
        "LLG_RAND_CONFLICTS",
        "LLG_RAND_ARRAY_ELEMENTS",
        "LLG_RAND_EXPAND_NODES",
    ] {
        command.env_remove(name);
    }
    // A node budget the 10-bit multiplier exceeds quickly and the 10-bit
    // adder fits in.
    command
        .env("LLG_RAND_BDD_NODES", "2000")
        .env("LLG_RAND_STATS", "1")
        .env("LLG_RAND_OVERFLOW_MEMO", memo)
        .env("LLG_RAND_SEARCH_CACHE", cache);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(300))
        .unwrap_or_else(|error| panic!("run overflow probe: {error}"));
    assert!(output.status.success(), "overflow probe failed: {output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stats = |zero: &str| {
        stderr
            .lines()
            .filter(|line| line.contains("randomize stats:") && !line.contains(zero))
            .count()
    };
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        stats("bdd_foreseen=0 "),
        stats("search_reused=0"),
    )
}

const BDD: u32 = 1 << 1;
const DIRECT: u32 = 1 << 0;

#[test]
fn foreseen_overflows_skip_only_builds_that_would_overflow() {
    let Some((_dir, exe)) = build_c_probe("constraint_overflow", OVERFLOW_MAIN) else {
        return;
    };
    let (with_both, foreseen, reused) = run_overflow_probe(&exe, "1", "1");
    let (without, foreseen_off, reused_off) = run_overflow_probe(&exe, "0", "0");
    // Neither shortcut changes a value, status or engine.
    assert_eq!(
        with_both, without,
        "the overflow memo or search cache changed the draws"
    );
    for (memo, cache) in [("1", "0"), ("0", "1")] {
        let (output, _, _) = run_overflow_probe(&exe, memo, cache);
        assert_eq!(
            output, without,
            "memo={memo} cache={cache} changed the draws"
        );
    }
    assert_eq!(
        foreseen_off, 0,
        "LLG_RAND_OVERFLOW_MEMO=0 still skipped builds"
    );
    assert_eq!(
        reused_off, 0,
        "LLG_RAND_SEARCH_CACHE=0 still reused encodings"
    );
    // Calls 1 (proof), 2 and 3 (repeating shape, new keys) and the new-key
    // calls after the exact, inactive and disabled interludes are skipped.
    assert!(
        foreseen >= 12,
        "the overflow memo was not exercised: {foreseen} skips"
    );
    // Keys searched a third time (calls 40 to 46) reuse their encoding.
    assert!(
        reused >= 2,
        "the search cache was not exercised: {reused} reuses"
    );

    let calls = parse_calls(&with_both);
    assert_eq!(calls.len(), 48);
    for (i, call) in calls.iter().enumerate() {
        assert_eq!(call.status, "ok", "call {i}: {}", call.status);
        if !call.block {
            assert_eq!(
                call.engines, DIRECT,
                "call {i}: no constraint, direct draws"
            );
            continue;
        }
        let x = if call.x_active { call.x } else { 7 };
        assert!(x > 1 && call.y > 1, "call {i}: x > 1 && y > 1");
        if call.g == 1 {
            assert_eq!(x * call.y, call.t, "call {i}: x * y == t");
        } else {
            assert_eq!(x + call.y, call.t, "call {i}: x + y == t");
        }
        if call.g == 0 || !call.x_active {
            // The state makes the exact model small: it must still be used
            // after the shape's overflow was proven.
            assert_ne!(call.engines & BDD, 0, "call {i}: exact engine expected");
        } else {
            assert_eq!(call.engines & BDD, 0, "call {i}: the multiplier overflows");
        }
    }
}
