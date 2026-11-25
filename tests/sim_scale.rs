//! Generation cost grows linearly with the instance count of a netlist and
//! with the class count of a class library.
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
/// Class counts of the small and large library, and the methods per class.
const SMALL_CLASSES: usize = 50;
const LARGE_CLASSES: usize = 4 * SMALL_CLASSES;
const METHODS: usize = 10;
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

/// The owned database of `source`, whose scratch directory is `name`.
fn database(name: &str, source: &str) -> Db {
    sim_harness::with_frontend_temp_cwd(name, |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, source).map_err(|error| error.to_string())?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())
    })
    .expect("design database")
}

/// One generation's wall time, coroutine body count and `model.c` size.
fn generate(db: &Db) -> (Duration, usize, usize) {
    let started = Instant::now();
    let model = sim::codegen::generate_from_db_with_opts(db, &OptConfig::default())
        .expect("design generation");
    let elapsed = started.elapsed();
    (
        elapsed,
        model.model_c.matches("LLG_CO_DISPATCH_BEGIN").count(),
        model.model_c.len(),
    )
}

/// Generate a design at two sizes and require the large one to take less
/// than `MAX_TIME_RATIO` times as long. Returns the coroutine body count and
/// `model.c` size of each.
fn assert_linear(names: [&str; 2], sources: [String; 2], sizes: &str) -> [(usize, usize); 2] {
    let designs = [
        database(names[0], &sources[0]),
        database(names[1], &sources[1]),
    ];
    let mut times = [Duration::MAX; 2];
    let mut outputs = [(0, 0); 2];
    // Alternate the sizes so host load affects both alike.
    for _ in 0..REPETITIONS {
        for (index, design) in designs.iter().enumerate() {
            let (time, bodies, bytes) = generate(design);
            times[index] = times[index].min(time);
            outputs[index] = (bodies, bytes);
        }
    }
    let ratio = times[1].as_secs_f64() / times[0].as_secs_f64();
    assert!(
        ratio < MAX_TIME_RATIO,
        "{sizes}: {:?} is {ratio:.1}x {:?}",
        times[1],
        times[0]
    );
    outputs
}

#[test]
fn netlist_generation_grows_linearly_with_instances() {
    let [(small_bodies, small_bytes), (large_bodies, large_bytes)] = assert_linear(
        ["scale_small_netlist", "scale_large_netlist"],
        [netlist(SMALL_CELLS), netlist(LARGE_CELLS)],
        &format!("{SMALL_CELLS} -> {LARGE_CELLS} cells"),
    );
    assert_eq!(
        small_bodies, large_bodies,
        "coroutine bodies must stay shared across instances"
    );
    // Deterministic: the generated text does not depend on the host.
    let per_cell = (large_bytes - small_bytes) / (LARGE_CELLS - SMALL_CELLS);
    assert!(
        per_cell <= MAX_C_BYTES_PER_CELL,
        "model.c grows by {per_cell} bytes per cell ({small_bytes} -> {large_bytes} bytes), above {MAX_C_BYTES_PER_CELL}"
    );
}

/// A class tree: every class overrides half of its methods from the root and
/// declares the other half, and one process calls some of both.
fn class_library(classes: usize) -> String {
    let mut source = String::from("package p;\n");
    for class in 0..classes {
        let base = match class {
            0 => String::new(),
            _ => format!(" extends c{}", (class - 1) / 4),
        };
        source.push_str(&format!("  class c{class}{base};\n    int v{class};\n"));
        for method in 0..METHODS / 2 {
            source.push_str(&format!(
                "    virtual function int f{method}(int a); return a + v{class} + {method}; endfunction\n\
                 \x20   virtual task t{class}_{method}(int a); if (a > {method}) v{class} = f{method}(a); endtask\n"
            ));
        }
        source.push_str("  endclass\n");
    }
    source.push_str("endpackage\nmodule tb;\n  import p::*;\n  c0 h;\n  initial begin\n");
    for class in 0..classes {
        source.push_str(&format!(
            "    begin c{class} x = new; h = x; x.t{class}_0(1); $display(\"%0d\", h.f0(3)); end\n"
        ));
    }
    source.push_str("  end\nendmodule\n");
    source
}

/// Virtual dispatch once scanned every function per class and slot, and calls
/// and `new` expressions scanned the classes or the whole database.
#[test]
fn class_generation_grows_linearly_with_classes() {
    assert_linear(
        ["scale_small_classes", "scale_large_classes"],
        [class_library(SMALL_CLASSES), class_library(LARGE_CLASSES)],
        &format!("{SMALL_CLASSES} -> {LARGE_CLASSES} classes"),
    );
}
