//! End-to-end simulator tests for hierarchical WRITES (3+ parts, e.g.
//! `tb.dut.sig = 8'h2a;` and `tb.dut.vec[3:0] = 4'hf;`): Surelog compile →
//! codegen → CMake build → run, like tests/sim_counter.rs.
//!
//! Reads of N-part hierarchical paths already work; these tests exercise the
//! write side: whole-signal blocking/NBA writes from a parent scope into a
//! child's reg, part-select/bit-select/indexed-part-select writes, and the
//! wake-on-write behaviour of a child process watching the target signal.
//! SystemVerilog implicit named (`.name`) and wildcard (`.*`) port
//! connections are also pinned after Surelog expands them.
//!
//! Surelog writes `slpp_all/` into the process working directory, so each test
//! runs with the CWD pointed at a fresh temp dir (serialized through a mutex).

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

/// Run `sv` in a fresh temp dir (holding the Surelog mutex) and assert the
/// exact stdout.
fn assert_stdout(tag: &str, sv: &str, expected: &str) {
    let stdout = sim_harness::run_sim(sv, "tb_top", tag).expect("simulation should run");
    assert_eq!(stdout, expected);
}

#[test]
fn sim_implicit_named_and_wildcard_port_connections() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ns
module child(input logic a, input logic b, output logic y);
    assign y = a ^ b;
endmodule

module named_child(input logic left, input logic right, output logic result);
    assign result = left & right;
endmodule

module tb_top;
    logic a;
    logic b;
    wire y;
    logic left;
    logic right;
    wire result;

    child u_wildcard (.*);
    named_child u_named (.left, .right, .result);

    initial begin
        a = 1'b1;
        b = 1'b0;
        left = 1'b1;
        right = 1'b1;
        #1 $display("wild=%0b named=%0b", y, result);
        $finish;
    end
endmodule
"#;
    assert_stdout("implicit_ports", sv, "wild=1 named=1\n");
}

/// (a) A 3-part blocking write from the top into a child's reg, visible to the
/// child through a direct global write (the child's `always @(reg1)` wakes on
/// the write and its initial reads the value later).
///
/// Hand-simulation (timescale 1ns/1ns):
///   t=0  dut's always@(reg1) registers a change waiter on reg1 (last-seen X);
///        dut's initial waits #3, then #4; tb_top's initial waits #2.
///   t=2  tb_top: u_tb.u_dut.reg1 = 8'h2a (blocking; X->2a change) -> the
///        child's always wakes and prints "watch reg1=2a"; tb_top waits #2.
///   t=3  dut's initial: prints "read reg1=2a"; waits #4.
///   t=4  tb_top: u_tb.u_dut.reg1 = 8'hff -> always prints "watch reg1=ff";
///        tb_top waits #10.
///   t=7  dut's initial: prints "read reg1=ff"; $finish.
///
/// Expected stdout (exactly):
///   watch reg1=2a
///   read reg1=2a
///   watch reg1=ff
///   read reg1=ff
#[test]
fn sim_hier_write_whole_blocking() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ns
module dut;
    reg [7:0] reg1;
    always @(reg1) $display("watch reg1=%0h", reg1);
    initial begin
        #3 $display("read reg1=%0h", reg1);
        #4 $display("read reg1=%0h", reg1);
        $finish;
    end
endmodule

module tb;
    dut u_dut();
endmodule

module tb_top;
    tb u_tb();
    initial begin
        #2 u_tb.u_dut.reg1 = 8'h2a;
        #2 u_tb.u_dut.reg1 = 8'hff;
        #10 $finish;
    end
endmodule
"#;
    assert_stdout(
        "hier_whole",
        sv,
        "watch reg1=2a\nread reg1=2a\nwatch reg1=ff\nread reg1=ff\n",
    );
}

/// (b) A hierarchical part-select write (`vec[3:0]`), a descending part-select
/// (`vec[7:4]`), a bit-select write (`vec[2]`) and an indexed part-select
/// (`v2[3 +: 4]`) on a child's regs; the child displays the final values.
///
/// Hand-simulation (timescale 1ns/1ns):
///   t=0  dut's initial: vec = 0, v2 = 0; waits #9.
///   t=2  tb_top: u_tb.u_dut.vec[3:0] = 4'hf -> vec = 0x0f.
///   t=4  tb_top: u_tb.u_dut.vec[7:4] = 4'ha -> vec = 0xaf.
///   t=6  tb_top: u_tb.u_dut.vec[2] = 1'b0 -> vec = 0xab (bit 2 cleared).
///   t=8  tb_top: u_tb.u_dut.v2[3 +: 4] = 4'h5 -> bits 6..3 = 0101, v2 = 0x28.
///   t=9  dut's initial: prints "vec=ab v2=28"; $finish.
///
/// Expected stdout (exactly):
///   vec=ab v2=28
#[test]
fn sim_hier_write_selects() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ns
module dut;
    reg [7:0] vec;
    reg [7:0] v2;
    initial begin
        vec = 8'h00;
        v2 = 8'h00;
        #9 $display("vec=%0h v2=%0h", vec, v2);
        $finish;
    end
endmodule

module tb;
    dut u_dut();
endmodule

module tb_top;
    tb u_tb();
    initial begin
        #2 u_tb.u_dut.vec[3:0] = 4'hf;
        #2 u_tb.u_dut.vec[7:4] = 4'ha;
        #2 u_tb.u_dut.vec[2] = 1'b0;
        #2 u_tb.u_dut.v2[3 +: 4] = 4'h5;
        #10 $finish;
    end
endmodule
"#;
    assert_stdout("hier_sel", sv, "vec=ab v2=28\n");
}

/// (c) A hierarchical NBA write with a clock: `always @(posedge clk)
/// u_tb.u_dut.cnt <= u_tb.u_dut.cnt + 1;` increments the child's reg once per
/// posedge (the RHS is a hierarchical read, the LHS a hierarchical write).
///
/// Hand-simulation (timescale 1ns/1ns):
///   t=0  dut's initial: cnt = 0.  tb_top: clk = 0; the counter registers its
///        posedge waiter (last-seen 0); always#5 waits t=5; initial waits #24.
///   t=5  clk 0->1 posedge -> cnt <= cnt + 1 (reads 0) -> NBA commits cnt=1.
///   t=15 posedge -> cnt <= 2.
///   t=24 initial: prints "cnt=2".
///   t=25 posedge -> cnt <= 3 (not printed).
///   t=34 initial: $finish.
///
/// Expected stdout (exactly):
///   cnt=2
#[test]
fn sim_hier_write_nba_clock() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ns
module dut;
    reg [3:0] cnt;
    initial cnt = 0;
endmodule

module tb;
    dut u_dut();
endmodule

module tb_top;
    reg clk;
    tb u_tb();
    always @(posedge clk) u_tb.u_dut.cnt <= u_tb.u_dut.cnt + 1;
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        #24 $display("cnt=%0d", u_tb.u_dut.cnt);
        #10 $finish;
    end
endmodule
"#;
    assert_stdout("hier_nba", sv, "cnt=2\n");
}

/// (d) v1 rejects hierarchical select indices/bounds that are not plain
/// integer literals (the db does not capture the select expression, so only
/// constant selects are recoverable).  The codegen must fail with a clear
/// message rather than silently mis-lower the write.
#[test]
fn sim_hier_write_rejects_variable_index() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ns
module dut;
    reg [7:0] vec;
endmodule

module tb;
    dut u_dut();
endmodule

module tb_top;
    integer i;
    tb u_tb();
    initial begin
        i = 2;
        u_tb.u_dut.vec[i] = 1'b1;
        $finish;
    end
endmodule
"#;
    let err = sim_harness::with_surelog_temp_cwd("hier_idx", |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb_top".to_string()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        sim::codegen::generate(design)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
    .expect_err("variable index must be rejected");
    assert!(
        err.to_string()
            .contains("constant integer indices/bounds only"),
        "unexpected codegen error: {err}"
    );
}
