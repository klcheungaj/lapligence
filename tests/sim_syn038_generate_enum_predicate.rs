//! SYN-038 public-CLI witnesses for enum equality in generated event and port contexts.

use crate::sim_cli;

#[test]
fn generated_enum_equality_event_and_child_predicate_match_in_both_modes() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "generate_enum_predicate",
        "events=1\nseen=1\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
