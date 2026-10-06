//! SYN-038 whole-union ref task calls through a child module hierarchy.
//!
//! The child task writes a named packed-struct field through its union ref
//! formal. A local call to the same task checks alias behavior independently
//! of the hierarchical call route. Both use the public CLI in both optimizer
//! modes under IEEE 1800-2009.

use crate::sim_cli;

#[test]
fn hierarchical_task_ref_updates_union_storage_and_named_view() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "union_hier_ref",
        "hier=12ff child-local=ab41 view=12/ff\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}
