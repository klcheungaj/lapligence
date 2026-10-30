//! The runtime records current-slot edge history for every signal change only
//! when the model has a reader of it: a clocking drive, a `##` cycle wait or
//! an assertion clock pulse. Every other model calls
//! `llg_rt_skip_clocking_history` at startup and pays no per-change entry
//! allocation or time-advance free. These checks are structural, so they stay
//! cheap and independent of host timing.

use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};

const SKIP: &str = "llg_rt_skip_clocking_history();";

fn render(name: &str, source: &str) -> String {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit(name, source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            ..Default::default()
        },
    )
    .expect("valid in-memory clocking history fixture");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    generate_from_db_with_codegen_options(&database, &CodegenOptions::default())
        .expect("fixture emits C")
        .model_c
}

/// The shape that exposed the cost: many instances whose input ports are
/// constant part-selects of one shared vector, plus an assertion whose
/// sequence clock is a plain edge (its tick entry is registered, not
/// recorded per change).
#[test]
fn designs_without_history_readers_skip_it() {
    let model = render(
        "plain.sv",
        r#"
module leaf(input logic clk, input logic [3:0] in);
  logic [3:0] cs = 0;
  always_ff @(posedge clk) cs <= in[0] ? cs + 4'd1 : cs;
endmodule
module tb;
  logic clk = 0;
  logic [31:0] lfsr = 32'h1;
  logic a = 0, b = 0;
  genvar i;
  for (i = 0; i < 4; i++) begin : g
    leaf u(clk, lfsr[i % 28 +: 4]);
  end
  always #1 clk = ~clk;
  always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
  assert property (@(posedge clk) a |-> ##1 b);
  initial #20 $finish;
endmodule
"#,
    );
    assert_eq!(model.matches(SKIP).count(), 1, "{model}");
    let start = model.find("int llg_model_start(").expect("model start");
    let init = model[start..]
        .find("llg_rt_init_with_args_and_precision(")
        .expect("runtime init");
    let skip = model[start..].find(SKIP).expect("skip call");
    assert!(
        init < skip,
        "the skip follows runtime init, which resets it"
    );
}

#[test]
fn clocking_cycle_waits_keep_history() {
    let model = render(
        "cycle.sv",
        r#"
module tb;
  logic clk = 0;
  default clocking cb @(posedge clk); endclocking
  always #1 clk = ~clk;
  initial begin
    ##0;
    ##2;
    $finish;
  end
endmodule
"#,
    );
    assert!(model.contains("llg_arm_clocking_cycle("), "{model}");
    assert!(!model.contains(SKIP), "{model}");
}

#[test]
fn clocking_drives_keep_history() {
    let model = render(
        "drive.sv",
        r#"
module tb;
  logic clk = 0;
  logic q;
  clocking cb @(posedge clk);
    output q;
  endclocking
  always #1 clk = ~clk;
  initial begin
    cb.q <= 1'b1;
    #4 $finish;
  end
endmodule
"#,
    );
    assert!(model.contains("llg_clocking_nba"), "{model}");
    assert!(!model.contains(SKIP), "{model}");
}

#[test]
fn assertion_clock_pulses_keep_history() {
    let model = render(
        "pulse.sv",
        r#"
module tb;
  logic a = 0, b = 0, c = 1;
  assert property (@(posedge a or posedge b) c);
  initial begin
    #1 a = 1;
    #1 b = 1;
    #1 $finish;
  end
endmodule
"#,
    );
    assert!(model.contains("llg_assertion_clock_pulse("), "{model}");
    assert!(!model.contains(SKIP), "{model}");
}
