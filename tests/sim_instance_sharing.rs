use crate::sim_cli;

/// `%m` names the enclosing named block, task or generate block as written
/// (SV 21.2.1.6).
#[test]
fn instance_paths_events_and_external_disable_stay_independent() {
    sim_cli::run_case(
        "instance_sharing",
        "identities",
        "start tb.u0.live\nstart tb.u1.live\nstart tb.u2.live\nstart tb.u3.live\ngenerate tb.g[0]\ngenerate tb.g[1]\ngenerate tb.g[2]\ngenerate tb.g[3]\ntask tb.u0.mark seen=1\nexit tb.u1\ntask tb.u2.mark seen=1\ntask tb.u3.mark seen=1\nseen 1 0 1 1 bits=1010\n",
        "", &[],
    );
}

/// Netlist cells connected to selected bus bits share their links, gates,
/// path stamps and UDP gates across instances; results equal the inverted
/// input (see the fixture readme).
#[test]
fn netlist_cell_bodies_are_shared_across_bus_bit_instances() {
    sim_cli::run_case(
        "instance_sharing",
        "netlist_cells",
        "15000 n=01011001 q=01011001\n28000 n=11110000 q=11110000\n",
        "",
        &[],
    );
    let (directory, output) =
        sim_cli::run_case_keeping_dir("instance_sharing", "netlist_cells", true, &[], &[]);
    assert!(output.status.success(), "{output:?}");
    let model = std::fs::read_to_string(directory.path().join("build/sim/tb/model.c"))
        .expect("generated model");
    // Nine shared classes (two links, the gate and the path stamp of each
    // cell, plus the flop's UDP gate) and the initial process; one body per
    // instance would be 57.
    assert_eq!(model.matches("LLG_CO_DISPATCH_BEGIN").count(), 10);
}
