//! Static declaration initializers of every kind keep declaration order
//! within their scope (decision S22-D8): patterns, class handles, strings,
//! containers and records interleaved with scalars, in a package, a class and
//! a module. Oracles are in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_initializer_order";

#[test]
fn every_initializer_kind_keeps_declaration_order() {
    sim_cli::run_case(
        SUITE,
        "kinds",
        include_str!("../fixtures/sim/feature_completion/ki_initializer_order/kinds.out"),
        "INFO: kinds.sv:89:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}
