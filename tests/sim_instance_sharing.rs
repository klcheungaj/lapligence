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

/// Every body kind of a loop generate construct shares one body across the
/// iterations: the function, the task, the inertial continuous assignment to
/// one bit of a common net and the initial process. Genvar payloads, driver
/// slots and each iteration's inertial handle are record operands (see the
/// fixture readme for the oracle).
#[test]
fn generate_iteration_bodies_share_one_body_per_kind() {
    sim_cli::run_case(
        "instance_sharing",
        "generate_bodies",
        "y=11111001\ny=01011001 q=10100110\nsum 0 33 77\n",
        "",
        &[],
    );
    let (directory, output) =
        sim_cli::run_case_keeping_dir("instance_sharing", "generate_bodies", true, &[], &[]);
    assert!(output.status.success(), "{output:?}");
    let model = std::fs::read_to_string(directory.path().join("build/sim/tb/model.c"))
        .expect("generated model");
    // The shared assignment, task and generate initial bodies, plus the top
    // initial process; one body per iteration would be 25.
    assert_eq!(model.matches("LLG_CO_DISPATCH_BEGIN").count(), 4);
    // One shared `x * 10 + i` function body for the eight iterations.
    assert_eq!(model.matches("sv4_mul").count(), 1);
    // Each iteration keeps its own inertial handle, at file scope.
    assert_eq!(model.matches("static llg_inertial_t* ").count(), 8);
    assert!(!model.contains("    static llg_inertial_t*"));
}

/// Gates annotated by SDF DEVICE entries share one body: each instance's
/// delay slot is a record operand, like its nets (see the fixture readme).
#[test]
fn sdf_annotated_gates_share_one_body() {
    sim_cli::run_case_with_inputs(
        "instance_sharing",
        "sdf_gates",
        &["sdf_gates.sdf"],
        "y=00001111\ny=11111111\n",
        "",
        &["--sdf", "sdf_gates.sdf"],
        &[],
    );
    let sdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/instance_sharing/sdf_gates.sdf");
    let sdf = sdf.to_string_lossy();
    let (directory, output) =
        sim_cli::run_case_keeping_dir("instance_sharing", "sdf_gates", true, &["--sdf", &sdf], &[]);
    assert!(output.status.success(), "{output:?}");
    let model = std::fs::read_to_string(directory.path().join("build/sim/tb/model.c"))
        .expect("generated model");
    // One gate body, the three port links and the initial process; one gate
    // body per instance would be 12.
    assert_eq!(model.matches("LLG_CO_DISPATCH_BEGIN").count(), 5);
}
