//! Generation cost grows linearly with the instance count of a netlist.
//!
//! Gate-level netlists hold 10^5..10^6 cell instances, so every generation
//! stage must stay near O(n). Lowering once scanned the whole design per
//! net, per dependency and per storage lookup; the regression showed as
//! quadruple time for doubled instances. This guard generates the same
//! netlist shape at two sizes and bounds the time ratio, and requires the
//! cells' process bodies to stay shared, so the C does not grow with
//! instances x body. The C added per cell is bounded too: per-instance
//! netlist data (nets, path and timing check sets, process records and
//! spawns) is emitted as compact table rows, not as initializer code.

use crate::sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::time::{Duration, Instant};

/// Instance counts of the small and large netlist (four times as many).
const SMALL_CELLS: usize = 300;
const LARGE_CELLS: usize = 4 * SMALL_CELLS;
/// Linear growth takes about 4x for 4x the cells (4.2x measured); the
/// quadratic lowering this guards against took 19x. The margin absorbs
/// loaded hosts; each size takes the fastest of `REPETITIONS` runs.
const MAX_TIME_RATIO: f64 = 8.0;
const REPETITIONS: usize = 2;
/// `model.c` bytes added per cell pair (an inverter, a UDP flop, their nets,
/// path sets, links and gates): 1409 measured, 13468 when each instance
/// emitted its own initializer code and descriptors.
const MAX_C_BYTES_PER_CELL: usize = 2000;

const CELLS: &str = r#"`timescale 1ns/1ps
primitive udp_dff (q, d, clk);
  output q;
  reg q;
  input d, clk;
  table
    // d clk : q : q+
       0  r  : ? : 0;
       1  r  : ? : 1;
       ?  f  : ? : -;
       *  ?  : ? : -;
  endtable
endprimitive

module dffc (input d, input clk, output q);
  udp_dff u (q, d, clk);
  specify
    (posedge clk => (q +: d)) = (1, 1);
  endspecify
endmodule

module inv (input a, output y);
  not g (y, a);
  specify
    (a => y) = (2, 3);
  endspecify
endmodule
"#;

/// A flat netlist: `cells` inverters on the bits of one bus, each feeding a
/// UDP flop, connected bit by bit with escaped instance names.
fn netlist(cells: usize) -> String {
    let mut source = String::from(CELLS);
    source.push_str(&format!(
        "module tb;\n  reg clk = 1'b0;\n  reg [{top}:0] din = '0;\n  wire [{top}:0] n;\n  wire [{top}:0] q;\n",
        top = cells - 1
    ));
    for cell in 0..cells {
        source.push_str(&format!(
            "  inv \\i_inv[{cell}] (.a(din[{cell}]), .y(n[{cell}]));\n  dffc \\r_reg[{cell}] (.d(n[{cell}]), .clk(clk), .q(q[{cell}]));\n"
        ));
    }
    source.push_str(
        "  initial begin\n    #10 clk = 1'b1;\n    #5 $display(\"%b\", ^q);\n    $finish(0);\n  end\nendmodule\n",
    );
    source
}

/// The owned database of `netlist(cells)`.
fn database(cells: usize) -> Db {
    sim_harness::with_frontend_temp_cwd(&format!("scale_{cells}"), |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, netlist(cells)).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())
    })
    .expect("netlist database")
}

/// One generation's wall time, coroutine body count and `model.c` size.
fn generate(db: &Db) -> (Duration, usize, usize) {
    let started = Instant::now();
    let model = sim::codegen::generate_from_db_with_opts(db, &OptConfig::default())
        .expect("netlist generation");
    let elapsed = started.elapsed();
    (
        elapsed,
        model.model_c.matches("LLG_CO_DISPATCH_BEGIN").count(),
        model.model_c.len(),
    )
}

#[test]
fn netlist_generation_grows_linearly_with_instances() {
    let small = database(SMALL_CELLS);
    let large = database(LARGE_CELLS);
    let mut small_time = Duration::MAX;
    let mut large_time = Duration::MAX;
    let mut bodies = (0, 0);
    let mut bytes = (0, 0);
    // Alternate the sizes so host load affects both alike.
    for _ in 0..REPETITIONS {
        let (time, count, size) = generate(&small);
        small_time = small_time.min(time);
        bodies.0 = count;
        bytes.0 = size;
        let (time, count, size) = generate(&large);
        large_time = large_time.min(time);
        bodies.1 = count;
        bytes.1 = size;
    }
    let ratio = large_time.as_secs_f64() / small_time.as_secs_f64();
    assert!(
        ratio < MAX_TIME_RATIO,
        "{LARGE_CELLS} cells took {large_time:?}, {ratio:.1}x the {small_time:?} of {SMALL_CELLS} cells"
    );
    assert_eq!(
        bodies.0, bodies.1,
        "coroutine bodies must stay shared across instances"
    );
    // Deterministic: the generated text does not depend on the host.
    let per_cell = (bytes.1 - bytes.0) / (LARGE_CELLS - SMALL_CELLS);
    assert!(
        per_cell <= MAX_C_BYTES_PER_CELL,
        "model.c grows by {per_cell} bytes per cell ({} -> {} bytes), above {MAX_C_BYTES_PER_CELL}",
        bytes.0,
        bytes.1
    );
}
