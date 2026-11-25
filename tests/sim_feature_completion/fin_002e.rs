//! FIN-002 step 4: conditional operator chains (IEEE 1800-2009 11.4.11) and
//! deeply nested designs.
//!
//! A chain of `?:` operators used to emit C that doubled with every level.
//! The fixtures run chains 4 to 32 levels deep with known, X and Z
//! predicates at every position; `fin_002e/readme.md` states where each
//! expected transcript comes from.
//!
//! | Fixture | What it holds |
//! | --- | --- |
//! | `cond_chain_packed` | priority chains in a continuous assignment, a procedure and a function; a two-state chain |
//! | `cond_chain_effects` | which predicates and alternatives are evaluated, and in which order |
//! | `cond_chain_types` | wide, signed, mixed-width, two-state, real, string, enumeration and packed structure operands |
//! | `cond_chain_shapes` | conditionals nested in the first alternative, in both, in the predicate; mixed types along a chain |
//! | `cond_chain_clause` | unpacked arrays and structures, class handles, reals and increments under the clause text |
//! | `deep_nesting` | every construct nested tens to hundreds of levels deep; `deep_nesting` in the `general` binary goes up to the limit without building |
//! | `recursive_specializations` | classes and a module that name themselves with other parameter values until the chain ends |
use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/fin_002e";

/// Upper bound on the size of one fixture's `model.c`. The largest holds
/// under half a megabyte; with code that doubles per level the 32-level
/// chains alone would need more than the address space.
const MODEL_SOURCE_LIMIT: usize = 1 << 20;

/// The fixtures end when no process remains; this is the run-time report.
const ENDED_AT_0: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 0\n";
const ENDED_AT_312: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 312\n";

macro_rules! expected {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/fin_002e/",
            $name,
            ".out"
        ))
    };
}

#[test]
fn priority_chains_select_and_merge_at_every_depth() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_packed",
        expected!("cond_chain_packed"),
        ENDED_AT_312,
        &[],
    );
}

#[test]
fn chain_alternatives_are_evaluated_once_in_source_order() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_effects",
        expected!("cond_chain_effects"),
        ENDED_AT_0,
        &[],
    );
}

#[test]
fn chains_keep_the_type_of_their_operands() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cond_chain_types",
        expected!("cond_chain_types"),
        &[],
        &[],
    );
}

#[test]
fn nested_conditionals_of_every_shape_agree() {
    sim_cli::run_case(
        SUITE,
        "cond_chain_shapes",
        expected!("cond_chain_shapes"),
        ENDED_AT_0,
        &[],
    );
}

#[test]
fn aggregate_handle_and_real_chains_follow_the_clause() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "cond_chain_clause",
        expected!("cond_chain_clause"),
        &[],
        &[],
    );
}

/// A model whose generation used to overflow the stack builds and runs. The C
/// compiler's optimizer is not under test, and takes longer than the rest.
#[test]
fn deeply_nested_constructs_build_and_run() {
    sim_cli::run_case_with_args(
        SUITE,
        "deep_nesting",
        expected!("deep_nesting"),
        "",
        &[],
        &["--model-opt-level", "O1"],
    );
}

#[test]
fn recursive_specializations_that_end_run() {
    sim_cli::run_case_with_args(
        SUITE,
        "recursive_specializations",
        expected!("recursive_specializations"),
        "",
        &[],
        &[],
    );
}

/// The generated C is proportional to the source: every fixture, with its
/// 32-level chains, stays far below the bound in both optimizer modes.
#[test]
fn generated_code_is_proportional_to_the_chain() {
    for fixture in [
        "cond_chain_packed",
        "cond_chain_effects",
        "cond_chain_types",
        "cond_chain_shapes",
        "cond_chain_clause",
    ] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(format!("{fixture}.sv"));
        for optimized in [true, false] {
            let directory = sim_harness::TempDir::new("conditional-chain").expect("scratch");
            let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
            command
                .args(["--gen-only", "--top", "tb", "--out-dir"])
                .arg(directory.path());
            if !optimized {
                command.arg("--no-opt");
            }
            let output = sim_harness::run_command(command.arg(&source), Duration::from_secs(60))
                .expect("generate the conditional chain model");
            assert!(output.status.success(), "{fixture}: {output:?}");
            let model = std::fs::read(directory.path().join("sim/tb/model.c")).expect("model.c");
            assert!(
                model.len() < MODEL_SOURCE_LIMIT,
                "{fixture}, optimized={optimized}: model.c has {} bytes",
                model.len()
            );
        }
    }
}
