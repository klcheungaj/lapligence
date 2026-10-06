//! Runtime module values consumed by static and automatic local initializers.

use crate::sim_cli;

#[test]
fn runtime_sources_initialize_static_and_automatic_locals() {
    sim_cli::run_case_with_args(
        "syn038_pairwise",
        "runtime_source_local_initializers",
        "runtime-local-init=39,39 color=5c,5c lanes=39,4a:39,4a\n",
        "llg: simulation ended without $finish (no processes remain) at time 0\n",
        &[],
        &["--edition", "2009"],
    );
}
