//! The runtime records current-slot edge history only for the storage a
//! reader names: the clock signal of a clocking drive and the pulse signal
//! of an assertion clock. Startup registers each one with
//! `llg_rt_clocking_history_signal`, at the address the reader's wait
//! sources take; a change of any other signal costs no history entry or
//! change record. `##` cycle waits wait on the clocking block's event, which
//! answers from its own triggered state, so they register nothing. These
//! checks are structural, so they stay cheap and independent of host timing.

use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};

const REGISTER: &str = "llg_rt_clocking_history_signal(";

/// The addresses startup registers, in order; each registration follows
/// runtime init, which resets the history table.
fn registrations(model: &str) -> Vec<String> {
    let start = model.find("int llg_model_start(").expect("model start");
    let start_body = &model[start..];
    let init = start_body
        .find("llg_rt_init_with_args_and_precision(")
        .expect("runtime init");
    let mut addresses = Vec::new();
    for (at, _) in start_body.match_indices(REGISTER) {
        assert!(init < at, "registration precedes runtime init: {model}");
        let rest = &start_body[at + REGISTER.len()..];
        addresses.push(rest[..rest.find(')').expect("call end")].to_owned());
    }
    assert_eq!(
        addresses.len(),
        model.matches(REGISTER).count(),
        "registrations outside startup: {model}"
    );
    addresses
}

/// Whether some reader's wait source array names `address`.
fn source_names(model: &str, address: &str) -> bool {
    model.contains(&format!("{{ .sig = {address}, .kind = "))
}

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
/// sequence clock is a plain edge (its tick entry registers itself at run
/// time, not at startup).
#[test]
fn designs_without_history_readers_register_nothing() {
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
    assert!(registrations(&model).is_empty(), "{model}");
}

/// `##N` waits on the clocking block's event, whose triggered state says
/// whether it occurred in this slot: no signal history is read.
#[test]
fn clocking_cycle_waits_register_nothing() {
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
    assert!(registrations(&model).is_empty(), "{model}");
}

/// A drive's clock is registered once, however many drives name it, and no
/// other signal is: neither the drive targets nor the written data.
#[test]
fn clocking_drives_register_their_clock_once() {
    let model = render(
        "drive.sv",
        r#"
module tb;
  logic clk = 0;
  logic q, r;
  logic [7:0] data = 0;
  clocking cb @(posedge clk);
    output q, r;
  endclocking
  always #1 clk = ~clk;
  always @(posedge clk) data <= data + 1;
  initial begin
    cb.q <= 1'b1;
    cb.r <= ##1 data[0];
    #4 $finish;
  end
endmodule
"#,
    );
    assert!(model.contains("llg_clocking_nba"), "{model}");
    let registered = registrations(&model);
    assert_eq!(registered.len(), 1, "{model}");
    assert!(registered[0].contains("clk"), "{model}");
    assert!(source_names(&model, &registered[0]), "{model}");
}

/// Each instance of an interface drives through its own clocking block, so
/// each instance's clock is registered, including drives reached through a
/// virtual interface.
#[test]
fn interface_instances_register_each_clock() {
    let model = render(
        "vif.sv",
        r#"
interface bus(input logic clk);
  logic q;
  clocking cb @(posedge clk);
    output q;
  endclocking
endinterface
class driver;
  virtual bus v;
  function new(virtual bus v);
    this.v = v;
  endfunction
  task run();
    v.cb.q <= 1'b1;
  endtask
endclass
module tb;
  logic c0 = 0, c1 = 0;
  bus b0(c0);
  bus b1(c1);
  always #1 c0 = ~c0;
  always #2 c1 = ~c1;
  initial begin
    driver d0 = new(b0);
    driver d1 = new(b1);
    d0.run();
    d1.run();
    b0.cb.q <= 1'b0;
    #8 $finish;
  end
endmodule
"#,
    );
    let registered = registrations(&model);
    assert_eq!(registered.len(), 2, "{model}");
    assert_ne!(registered[0], registered[1], "{model}");
    for address in &registered {
        assert!(source_names(&model, address), "{address}: {model}");
    }
}

/// An assertion clock on several signals is a hidden signal the clocking
/// event pulses; the pulse asks whether that signal already rose in this
/// slot.
#[test]
fn assertion_clock_pulses_register_their_signal() {
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
    let registered = registrations(&model);
    assert_eq!(registered.len(), 1, "{model}");
    assert!(
        model.contains(&format!("llg_assertion_clock_pulse({});", registered[0])),
        "{model}"
    );
}
