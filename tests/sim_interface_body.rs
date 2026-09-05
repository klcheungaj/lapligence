//! End-to-end simulator tests for interface body processes: always/initial/
//! always_comb blocks declared INSIDE an interface definition.
//!
//! Surelog v1.86 elaborates the definition's processes onto the ACTUAL
//! interface instance only (`top.u_bus`), never onto the per-port copies
//! (`top.u_cons.s`), which are just views kept in sync by the interface link
//! processes.  The codegen emits the processes for the actual instance and
//! skips the copies.
//!
//! Surelog writes `slpp_all/` into the process working directory, so the tests
//! run with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! like the other Surelog integration tests).

use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile, codegen, build and run `sv` (top module `top`); returns stdout.
/// Asserts the codegen emitted no "interface body process skipped" warning.
fn run_design(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join(format!("{tag}.sv"));
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("top".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        let gen = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;
        assert!(
            !gen.warnings
                .iter()
                .any(|w| w.contains("interface body process skipped")),
            "interface body processes must not be skipped with a warning: {:?}",
            gen.warnings
        );
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        sim_harness::run_executable(&exe)
    })
}

/// (a) Clock generator inside an interface: the interface's `initial` seeds
/// the clock, its `always #5` toggles it, and a consumer connected through a
/// `slave(input clk)` modport counts posedges.  The definition's processes
/// run on the ACTUAL instance (`top.u_bus`); the input modport link pushes
/// each toggle into the consumer's per-port copy (`top.u_cons.s`), so the
/// consumer sees the clock.
///
/// Hand-simulated trace (default timescale 1ns/1ps; spawn order at t=0:
/// comb, links, then procs — the top's `initial` spawns before the child
/// instances' processes because `emit_pass_inst` emits a scope's own
/// processes before recursing into child instances):
///   t=0  interface `initial`: u_bus.clk = 0; consumer `initial`: cnt = 0.
///        u_bus `always #5`: waits 5 ns; consumer: waits posedge clk;
///        top initial: waits #10.
///   t=5  u_bus always: clk 0→1 (posedge); consumer: cnt <= 1.
///   t=10 top initial wakes (enqueued at t=0, before the always's t=5
///        wakeup): displays "10 cnt=1 clk=1", waits #15 → t=25.
///        u_bus always: clk 1→0.
///   t=15 clk 0→1 (posedge); cnt <= 2.
///   t=20 clk 1→0.
///   t=25 top initial wakes first (enqueued at t=10): displays
///        "25 cnt=2 clk=0", waits #15 → t=40.  u_bus always: clk 0→1;
///        cnt <= 3.
///   t=30 clk 1→0.
///   t=35 clk 0→1 (posedge); cnt <= 4.
///   t=40 top initial wakes first (enqueued at t=25): displays
///        "40 cnt=4 clk=1"; $finish.
///
/// Expected stdout (exactly):
///   10 cnt=1 clk=1
///   25 cnt=2 clk=0
///   40 cnt=4 clk=1
#[test]
fn sim_iface_body_clock_gen() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"interface bus_if;
    logic clk;
    modport slave(input clk);
    initial clk = 0;
    always #5 clk = ~clk;
endinterface

module consumer(bus_if.slave s, output logic [3:0] cnt);
    initial cnt = 0;
    always @(posedge s.clk) begin
        cnt <= cnt + 1;
    end
endmodule

module top;
    logic [3:0] cnt;
    bus_if u_bus();
    consumer u_cons(.s(u_bus.slave), .cnt(cnt));
    initial begin
        #10 $display("%0t cnt=%0d clk=%b", $time, cnt, u_bus.clk);
        #15 $display("%0t cnt=%0d clk=%b", $time, cnt, u_bus.clk);
        #15 $display("%0t cnt=%0d clk=%b", $time, cnt, u_bus.clk);
        $finish;
    end
endmodule
"#;
    let stdout = run_design(sv, "clock_gen").expect("clock-gen design should run");
    assert_eq!(stdout, "10 cnt=1 clk=1\n25 cnt=2 clk=0\n40 cnt=4 clk=1\n");
}

/// (b) Combinational logic inside an interface: the interface's `always_comb`
/// drives `data`, and a consumer connected through a `slave(input data)`
/// modport forwards it onto a plain output port.  The modport direction is
/// from the connected module's perspective (LRM 25.5, matching
/// `tests/sim_interface.rs`): `input data` means the interface outputs data
/// into the consumer's per-port copy.
///
/// Hand-simulated trace (spawn order at t=0: comb, links, procs):
///   t=0  u_bus always_comb (no reads): u_bus.data = 0x2A, runs once.
///        u_cons always_comb: dout = s.data (copy still X) → X; waits on
///        {s.data}.  Input link (u_bus.data → copy_s.data) copies 0x2A →
///        u_cons comb re-runs → dout = 0x2A; output link (u_cons.dout →
///        top.dout) copies 0x2A.  Top initial waits #1.
///   t=1  initial: $display("dout=2a"); $finish.
///
/// Expected stdout (exactly):
///   dout=2a
#[test]
fn sim_iface_body_comb_data() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"interface bus_if;
    logic [7:0] data;
    modport slave(input data);
    always_comb data = 8'h2A;
endinterface

module consumer(bus_if.slave s, output logic [7:0] dout);
    always_comb dout = s.data;
endmodule

module top;
    logic [7:0] dout;
    bus_if u_bus();
    consumer u_cons(.s(u_bus.slave), .dout(dout));
    initial begin
        #1 $display("dout=%0h", dout);
        $finish;
    end
endmodule
"#;
    let stdout = run_design(sv, "comb_data").expect("comb-data design should run");
    assert_eq!(stdout, "dout=2a\n");
}

/// (c) `initial` block inside an interface: it drives a member that a
/// consumer reads through a `slave(input rst)` modport, so the value is 1
/// from t=0 onward.
///
/// Hand-simulated trace (spawn order at t=0: comb, links, procs):
///   t=0  u_cons always_comb: r = s.rst (copy X) → X; waits on {s.rst}.
///        Input link (u_bus.rst → copy_s.rst) copies X.  Output link
///        (u_cons.r → top.r) copies X.  Procs: interface `initial`
///        rst = 1 → input link wakes → copy_s.rst = 1 → u_cons comb re-runs
///        → r = 1 → output link wakes → top.r = 1.  Top initial waits #1.
///   t=1  initial: $display("rst=1"); $finish.
///
/// Expected stdout (exactly):
///   rst=1
#[test]
fn sim_iface_body_initial() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"interface bus_if;
    logic rst;
    modport slave(input rst);
    initial rst = 1'b1;
endinterface

module consumer(bus_if.slave s, output logic r);
    always_comb r = s.rst;
endmodule

module top;
    logic r;
    bus_if u_bus();
    consumer u_cons(.s(u_bus.slave), .r(r));
    initial begin
        #1 $display("rst=%b", r);
        $finish;
    end
endmodule
"#;
    let stdout = run_design(sv, "initial").expect("initial design should run");
    assert_eq!(stdout, "rst=1\n");
}
