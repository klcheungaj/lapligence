//! Storage that the unused-storage optimizer must keep: variables read only
//! through event-handle indices, handle receivers and arguments, and the wait
//! sources of clocked nonblocking operations.

use crate::sim_cli;

const SUITE: &str = "opt_storage";

#[test]
fn indirectly_referenced_storage_survives_the_optimizer() {
    sim_cli::run_case(
        SUITE,
        "indirect_references",
        "ev[0].triggered after -> = 1\n\
         method call on item 7\n\
         handle argument item 7\n\
         woke at 1 2 3; late fired 0; never = 0\n",
        "llg: $finish at time 4 at tb:64:5\n",
        &[],
    );
}
