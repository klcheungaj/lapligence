//! Public CLI coverage for combinational UDP truth tables and explicit UDP
//! rejection boundaries. Each fixture runs with and without optimization.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile::{self, CompileOpts, OwnedSource};
use llg::core::db::{NodeKind, PrimClass};

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
        wire out;
        owned_udp u(out, a, b);
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
}

#[test]
fn combinational_udp_truth_table_and_drivers() {
    const EXPECTED: &str = "known 0\nknown 1\nknown 1\nknown 0\nx 1\nz 1\nunmatched x\nb_symbol x\nwildcard x\narray 10\nresolved 0\nresolved x\nresolved_z x\ndelay_before 1\ndelay_after 0\n";
    sim_cli::run_case(
        "partial_features",
        "udp_comb",
        EXPECTED,
        "",
        &[],
    );
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
fn sequential_udp_remains_rejected() {
    sim_cli::reject_case(
        "partial_features",
        "udp_sequential_rejected",
        "user-defined primitive instance is not supported",
    );
}

#[test]
fn edge_sensitive_udp_row_remains_rejected() {
    sim_cli::reject_case(
        "partial_features",
        "udp_edge_rejected",
        "combinational UDP row contains state or edge metadata",
    );
}
