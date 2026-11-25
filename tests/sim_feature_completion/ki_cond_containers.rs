//! Conditional operators over queues, dynamic and associative arrays and
//! records with string, handle or container members (IEEE 1800-2009
//! 11.4.11), nested 4 to 32 deep.
//!
//! Nested conditionals of these types used to lower to code that doubled
//! with every level. `ki_cond_containers/readme.md` states where each expected
//! transcript comes from.
//!
//! | Fixture | What it holds |
//! | --- | --- |
//! | `chain_containers` | chains over each container kind, bounded queues, wide and two-state elements; every predicate position 1, X or Z |
//! | `chain_effects` | which predicates and alternatives are evaluated, in which order; a target that is one of its own alternatives |
//! | `chain_shapes` | conditionals nested in the first alternative, in both, as a tree |
//! | `chain_records_known` | records with string, real, handle, container, nested and array members under known predicates |
//! | `chain_records` | the same under every predicate, with nonblocking, element and continuous targets, a tagged union and conditionals in call arguments |
//! | `chain_arguments` | conditionals over records in call arguments, 5 to 8 levels deep, with callees that write the alternatives and the target |
//! | `chain_late_reads` | a later alternative that writes an earlier one |
//! | `chain_predicates` | vector, signed, real, string, constant and fill predicates |
//! | `chain_patterns` | `matches` predicates whose identifiers the alternatives use |
use super::{sim_cli, sim_harness};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/ki_cond_containers";

/// Upper bound on the size of one fixture's `model.c`; code that doubles per
/// level would need more than the address space for the 32-level chains.
const MODEL_SOURCE_LIMIT: usize = 4 << 20;

const ENDED: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 0\n";
const ENDED_AT_37000: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 37000\n";
const ENDED_AT_19000: &str =
    "INFO: [SIM-022] simulation ended without $finish (no processes remain) at time 19000\n";

macro_rules! expected {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/ki_cond_containers/",
            $name,
            ".out"
        ))
    };
}

#[test]
fn container_chains_select_and_merge_at_every_depth() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "chain_containers",
        expected!("chain_containers"),
        &[],
        &[],
    );
}

#[test]
fn alternatives_are_evaluated_once_in_source_order_and_may_read_the_target() {
    sim_cli::run_case(
        SUITE,
        "chain_effects",
        expected!("chain_effects"),
        ENDED,
        &[],
    );
}

#[test]
fn nested_conditionals_of_every_shape_agree() {
    sim_cli::run_case(SUITE, "chain_shapes", expected!("chain_shapes"), ENDED, &[]);
}

#[test]
fn conditionals_nest_through_call_arguments() {
    sim_cli::run_case(
        SUITE,
        "chain_arguments",
        expected!("chain_arguments"),
        ENDED_AT_37000,
        &[],
    );
}

#[test]
fn record_chains_select_every_member() {
    sim_cli::run_case(
        SUITE,
        "chain_records_known",
        expected!("chain_records_known"),
        ENDED,
        &[],
    );
}

#[test]
fn record_chains_merge_member_by_member() {
    sim_cli::run_case(
        SUITE,
        "chain_records",
        expected!("chain_records"),
        ENDED_AT_19000,
        &[],
    );
}

#[test]
fn a_record_alternative_is_read_after_the_alternatives_that_write_it() {
    sim_cli::run_case(
        SUITE,
        "chain_late_reads",
        expected!("chain_late_reads"),
        ENDED,
        &[],
    );
}

#[test]
fn every_predicate_type_selects_at_every_depth() {
    sim_cli::run_case(
        SUITE,
        "chain_predicates",
        expected!("chain_predicates"),
        ENDED,
        &[],
    );
}

#[test]
fn pattern_predicates_bind_their_identifiers_at_every_depth() {
    sim_cli::run_case(
        SUITE,
        "chain_patterns",
        expected!("chain_patterns"),
        ENDED,
        &[],
    );
}

/// The generated C is proportional to the source in both optimizer modes.
#[test]
fn generated_code_is_proportional_to_the_chain() {
    for fixture in [
        "chain_containers",
        "chain_effects",
        "chain_shapes",
        "chain_records_known",
        "chain_records",
        "chain_arguments",
        "chain_late_reads",
        "chain_predicates",
        "chain_patterns",
    ] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim")
            .join(SUITE)
            .join(format!("{fixture}.sv"));
        for optimized in [true, false] {
            let directory = sim_harness::TempDir::new("container-conditional").expect("scratch");
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
