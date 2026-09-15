//! Regressions for review batch 03. Each case uses strict output comparison
//! in both optimizer modes. Their presence is not a recorded test result.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn clocking_event() {
    sim_cli::run_case(
        "review_batch3",
        "clocking_event",
        "clocking event ok\n",
        "",
        &[],
    );
}

#[test]
fn constructor_order() {
    sim_cli::run_case(
        "review_batch3",
        "constructor_order",
        "constructor order ok\n",
        "",
        &[],
    );
}

#[test]
fn factory_receiver() {
    sim_cli::run_case(
        "review_batch3",
        "factory_receiver",
        "factory receiver ok\n",
        "",
        &[],
    );
}

#[test]
fn class_nonpacked() {
    sim_cli::run_case(
        "review_batch3",
        "class_nonpacked",
        "class nonpacked ok\n",
        "",
        &[],
    );
}

#[test]
fn assert_controls() {
    sim_cli::run_case(
        "review_batch3",
        "assert_controls",
        "assertion controls ok\n",
        "",
        &[],
    );
}

#[test]
fn deferred_controls() {
    sim_cli::run_case(
        "review_batch3",
        "deferred_controls",
        "deferred controls ok\n",
        "",
        &[],
    );
}

#[test]
fn property_truth() {
    sim_cli::run_case(
        "review_batch3",
        "property_truth",
        "property truth ok\n",
        "",
        &[],
    );
}

#[test]
fn scan_prefix() {
    sim_cli::run_case(
        "review_batch3",
        "scan_prefix",
        "numeric scanner ok\n",
        "",
        &[],
    );
}

#[test]
fn descriptor_namespaces() {
    sim_cli::run_case(
        "review_batch3",
        "descriptor_namespaces",
        "mcd fanout\nstandard stdout\ndescriptor namespaces ok\n",
        "",
        &[],
    );
}
