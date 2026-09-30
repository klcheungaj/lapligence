//! Public CLI coverage for combinational UDP truth tables and explicit UDP
//! rejection boundaries. Each fixture runs with and without optimization.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile::{self, CompileOpts, OwnedSource};
use llg::core::db::{NodeKind, PrimClass};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn031_combinational_udp")
        .join(name)
}

fn run_verilog_2001(source: &std::path::Path, expected: &str) {
    assert!(
        llg::sim::build::cmake_available(),
        "CLI tests require CMake"
    );
    for optimized in [false, true] {
        let directory = sim_harness::TempDir::new("syn031-2001").expect("CLI test directory");
        let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
        command
            .current_dir(directory.path())
            .args(["--top", "tb", "--edition", "2001"]);
        if !optimized {
            command.arg("--no-opt");
        }
        command.arg(source);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(180))
            .expect("Verilog 2001 model run");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
}

#[test]
fn db_owns_combinational_udp_rows() {
    let source = r#"primitive owned_udp(out, a, b);
        output out;
        input a, b;
        table
            0 0 : 0;
            0 1 : 1;
            x ? : x;
        endtable
    endprimitive
    module tb;
        reg a, b;
        wire out, other;
        wire [1:0] array_out;
        owned_udp u(out, a, b);
        owned_udp v(other, a, b);
        owned_udp array[1:0](array_out, a, b);
    endmodule
    "#;
    let compiled = compile::compile_sources_checked(
        &[OwnedSource::compilation_unit("udp_owned.sv", source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..CompileOpts::default()
        },
    )
    .expect("UDP source should compile");
    let db = llg::core::db::Db::from_slang(&compiled.snapshot).expect("owned database");
    drop(compiled);
    let table = db.nodes().iter().find_map(|node| match &node.kind {
        NodeKind::Gate {
            class: PrimClass::Udp,
            udp: Some(table),
            ..
        } => Some(table),
        _ => None,
    });
    let table = table.expect("UDP instance should retain its owned table");
    assert_eq!(table.name, "owned_udp");
    assert_eq!(table.input_count, 2);
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| (row.inputs.as_str(), row.output))
            .collect::<Vec<_>>(),
        vec![("00", b'0'), ("01", b'1'), ("x?", b'x')]
    );
    let generated = llg::sim::codegen::generate(&db)
        .expect("UDP should lower after native snapshot is dropped");
    assert_eq!(
        generated
            .model_c
            .matches("static const uint8_t llg_udp_table_")
            .count(),
        1
    );
}

#[test]
fn combinational_udp_truth_table_and_drivers() {
    const EXPECTED: &str = "known 0\nknown 1\nknown 1\nknown 0\nx 1\nz 1\nunmatched x\nb_symbol x\nwildcard x\narray 10\nresolved 0\nresolved x\nresolved_z x\ndelay_before 1\ndelay_after 0\n";
    sim_cli::run_case("partial_features", "udp_comb", EXPECTED, "", &[]);
    sim_cli::run_case_with_args(
        "partial_features",
        "udp_comb",
        EXPECTED,
        "",
        &[],
        &["--edition", "2001"],
    );
}

#[test]
fn syn_031_combinational_udp_matrix_both_editions() {
    // V 8.1.6/8.2 and SV 29.3.5/29.4: b=0|1, ?=0|1|x; input Z acts as X;
    // unmatched rows return X. V 8.6 / SV 29.8 allow arrays and delays.
    const EXPECTED: &str = concat!(
        "start mux=0/0 alt=1/1 parity=0/0 array=11/11 resolved=0/0 delay=x/x\n",
        "delay0 0/0\n",
        "changed mux=1/1 alt=0/0 parity=1/1 array=10/10 resolved=1/1 delay=0/0\n",
        "delay1 1/1\n",
        "same_known mux=0/0 parity=x/x array=x0/x0 resolved=x/x delay=0/0\n",
        "z_control mux=1/1 parity=x/x resolved=1/1\n",
        "unmatched mux=x/x alt=x/x parity=x/x resolved=x/x\n",
        "z_input mux=x/x parity=x/x resolved=x/x\n",
        "wildcard mux=0/0 parity=0/0 resolved=0/0\n",
    );
    run_verilog_2001(&fixture("syn_031_combinational_udp.v"), EXPECTED);
    sim_cli::run_case_with_args(
        "syn031_combinational_udp",
        "syn_031_combinational_udp",
        EXPECTED,
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn syn_031_invalid_ports_and_table_width_reject_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            "syn031_combinational_udp",
            "invalid_port_list",
            "port 'missing' is missing a corresponding body declaration",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            "syn031_combinational_udp",
            "invalid_table_width",
            "incorrect number of input fields in table row; have 1 but expect 2",
            &["--edition", edition],
        );
    }
}

#[test]
fn sequential_udp_remains_rejected() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            "partial_features",
            "udp_sequential_rejected",
            "user-defined primitive instance is not supported",
            &["--edition", edition],
        );
    }
}

#[test]
fn edge_sensitive_udp_row_remains_rejected() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            "partial_features",
            "udp_edge_rejected",
            "combinational UDP row contains state or edge metadata",
            &["--edition", edition],
        );
    }
}

#[test]
fn combinational_udp_overlapping_masks_and_state_matrix() {
    const EXPECTED: &str = concat!(
        "00 1\n01 1\n0x 1\n0z 1\n10 1\n11 1\n1x 1\n1z 1\n",
        "x0 0\nx1 x\nxx x\nxz x\nz0 0\nz1 x\nzx x\nzz x\narray 110x\n",
    );
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "partial_features",
            "udp_masks",
            EXPECTED,
            "",
            &[],
            &["--edition", edition],
        );
    }
}
