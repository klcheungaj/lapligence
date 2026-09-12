//! End-to-end four-state tests for ordinary logical implication and equivalence.
//!
//! The checked-in fixture exercises all 0/1/X/Z operand pairs, vector truth
//! reduction, precedence and associativity, real operands, and side-effect
//! evaluation. The shared CLI harness runs each oracle with and without the
//! optimizer.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn implication_and_equivalence_match_four_state_oracles() {
    sim_cli::run_case(
        "logical_ops",
        "implication_equivalence",
        concat!(
            "imp0=1111\n",
            "imp1=01xx\n",
            "impx=x1xx\n",
            "impz=x1xx\n",
            "eq0=10xx\n",
            "eq1=01xx\n",
            "eqx=xxxx\n",
            "eqz=xxxx\n",
            "mixed=01\n",
            "short_false=1 calls=0\n",
            "short_true=0 calls=1\n",
            "short_unknown=x calls=1\n",
            "equiv_calls=0 calls=2\n",
            "precedence=1 chain=1\n",
            "real=1x decl=11\n",
        ),
        "llg: $finish at time 0 at tb:79:9\n",
        &[],
    );
}
