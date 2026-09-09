//! End-to-end simulator tests for interface / modport simulation.
//!
//! These tests temporarily change the process working directory, so the tests
//! run with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! to avoid process-wide CWD races).

#[path = "support/sim.rs"]
mod sim_harness;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn run_design(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "top", tag)
}

/// Modport propagation through both directions: the master writes its per-port
/// copy (`u_m.m`), the output links push the values into the actual interface
/// (`u_bus`), the input links pull them into the slave's copy (`u_s.s`), and
/// the slave's always_comb forwards them onto its output ports, which the
/// plain port links copy into `top`.
///
/// Design (W=8, data + valid, master output / slave input modports):
/// ```systemverilog
/// interface bus_if #(parameter int W = 8);
///     logic [W-1:0] data;
///     logic valid;
///     modport master (output data, output valid);
///     modport slave (input data, input valid);
/// endinterface
/// ```
/// The master drives constants (`m.data = 8'h2A; m.valid = 1;`) so the trace
/// is deterministic (no X arithmetic).
///
/// Hand-simulated trace (spawn order at t=0: links, then procs):
///   t=0  links spawn: u_bus.data=X ← copy_m.data=X; u_bus.valid=X;
///        copy_s.data=X ← u_bus.data; copy_s.valid=X ← u_bus.valid;
///        top.dout=X ← u_s.dout; top.vout=X ← u_s.vout.
///        u_m always_comb: copy_m.data=X→0x2A, copy_m.valid=X→1; no reads →
///        runs once and ends (codegen warns).
///        u_s always_comb: u_s.dout=X (reads copy_s.data=X),
///        u_s.vout=X; waits on {copy_s.data, copy_s.valid}.
///        top initial: #1 → wakes at t=1.
///        Scheduler re-runs woken links: copy_m.data 0x2A → u_bus.data=0x2A →
///        copy_s.data=0x2A → u_s comb wakes → u_s.dout=0x2A → top.dout=0x2A;
///        copy_m.valid 1 → u_bus.valid=1 → copy_s.valid=1 → u_s.vout=1 →
///        top.vout=1.  Everything settles within t=0.
///   t=1  initial: $display("dout=2a vout=1"); #5 → t=6.
///   t=6  initial: $display("dout=2a vout=1"); $finish.
///
/// Expected stdout (exactly):
///   dout=2a vout=1
///   dout=2a vout=1
#[test]
fn sim_interface_modport_propagation() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"interface bus_if #(parameter int W = 8);
    logic [W-1:0] data;
    logic valid;
    modport master (output data, output valid);
    modport slave (input data, input valid);
endinterface

module ifc_master (bus_if.master m);
    always_comb begin
        m.data = 8'h2A;
        m.valid = 1;
    end
endmodule

module ifc_slave (bus_if.slave s, output logic [7:0] dout, output logic vout);
    always_comb begin
        dout = s.data;
        vout = s.valid;
    end
endmodule

module top;
    logic [7:0] dout;
    logic vout;
    bus_if #(.W(8)) u_bus();
    ifc_master u_m(.m(u_bus.master));
    ifc_slave u_s(.s(u_bus.slave), .dout(dout), .vout(vout));
    initial begin
        #1 $display("dout=%0h vout=%b", dout, vout);
        #5 $display("dout=%0h vout=%b", dout, vout);
        $finish;
    end
endmodule
"#;
    let stdout = run_design(sv, "modport").expect("modport design should run");
    assert_eq!(stdout, "dout=2a vout=1\ndout=2a vout=1\n");
}

/// Interface instance with an overridden parameter width: `#(.W(4))` folds
/// the interface vars (and the slave's output port) to 4 bits, and the master
/// drives a 4-bit value through the modport output link.
///
/// Hand-simulated trace (same shape as the modport test, W=4):
///   t=0  links spawn (all-X); u_m always_comb writes copy_m.data=4'ha
///        (0xa), copy_m.valid=1; the output links push 0xa → u_bus.data and
///        1 → u_bus.valid; the input links pull them into copy_s; u_s
///        always_comb forwards u_s.dout=4'ha, u_s.vout=1; the plain port
///        links copy those to top.dout / top.vout.  Settles within t=0.
///   t=1  initial: $display("dout=a vout=1"); $finish.
///
/// Expected stdout (exactly):
///   dout=a vout=1
#[test]
fn sim_interface_param_width() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"interface bus_if #(parameter int W = 4);
    logic [W-1:0] data;
    logic valid;
    modport master (output data, output valid);
    modport slave (input data, input valid);
endinterface

module ifc_master (bus_if.master m);
    always_comb begin
        m.data = 4'ha;
        m.valid = 1;
    end
endmodule

module ifc_slave (bus_if.slave s, output logic [3:0] dout, output logic vout);
    always_comb begin
        dout = s.data;
        vout = s.valid;
    end
endmodule

module top;
    logic [3:0] dout;
    logic vout;
    bus_if #(.W(4)) u_bus();
    ifc_master u_m(.m(u_bus.master));
    ifc_slave u_s(.s(u_bus.slave), .dout(dout), .vout(vout));
    initial begin
        #1 $display("dout=%0h vout=%b", dout, vout);
        $finish;
    end
endmodule
"#;
    let stdout = run_design(sv, "param_width").expect("param-width design should run");
    assert_eq!(stdout, "dout=a vout=1\n");
}
