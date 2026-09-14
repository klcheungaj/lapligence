//! Static-review regression sources for the eight remaining findings.
//! These cases have not been executed as part of the patch delivery.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn queue_outdated() {
    sim_cli::run_case("review_batch4", "queue_outdated", "queue outdated references ok\n", "", &[]);
}

#[test]
fn queue_ref_lifetime() {
    sim_cli::run_case("review_batch4", "queue_ref_lifetime", "queue reference lifetime ok\n", "", &[]);
}

#[test]
fn mailbox_nominal() {
    sim_cli::run_case("review_batch4", "mailbox_nominal", "mailbox nominal types ok\n", "", &[]);
}

#[test]
fn mailbox_equivalent() {
    sim_cli::run_case("review_batch4", "mailbox_equivalent", "mailbox equivalence and ref destination ok\n", "", &[]);
}

#[test]
fn empty_sequences() {
    sim_cli::run_case("review_batch4", "empty_sequences", "empty sequence boundaries ok\n", "", &[]);
}

#[test]
fn repetition_guards() {
    sim_cli::run_case("review_batch4", "repetition_guards", "guarded repetition endpoints ok\n", "", &[]);
}

#[test]
fn first_match_scope() {
    sim_cli::run_case("review_batch4", "first_match_scope", "first_match scope ok\n", "", &[]);
}

#[test]
fn first_match_ties() {
    sim_cli::run_case("review_batch4", "first_match_ties", "first_match tied endpoints ok\n", "", &[]);
}

#[test]
fn multiclock_time() {
    sim_cli::run_case("review_batch4", "multiclock_time", "multiclock physical time ok\n", "", &[]);
}

#[test]
fn multiclock_flow() {
    sim_cli::run_case("review_batch4", "multiclock_flow", "multiclock flow ok\n", "", &[]);
}

#[test]
fn implication_locals() {
    sim_cli::run_case("review_batch4", "implication_locals", "implication local transfer ok\n", "", &[]);
}

#[test]
fn implication_branches() {
    sim_cli::run_case("review_batch4", "implication_branches", "implication branching endpoints ok\n", "", &[]);
}

#[test]
fn packed_prefixes() {
    sim_cli::run_case("review_batch4", "packed_prefixes", "packed prefix dependencies ok\n", "", &[]);
}

#[test]
fn packed_array_prefixes() {
    sim_cli::run_case("review_batch4", "packed_array_prefixes", "packed array prefixes ok\n", "", &[]);
}

#[test]
fn overlapping_packed_prefixes_still_reject() {
    sim_cli::reject_case("review_batch4", "packed_overlap", "multiple");
}
