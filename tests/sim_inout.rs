//! End-to-end simulator tests for inout ports / tri-state nets: parent and
//! child nets through an inout port collapse into one resolved simulated net
//! (LRM §23.3.3.7) with per-driver strength-aware resolution across wire/tri
//! and wired net kinds.
//!
//! These tests temporarily change the process working directory, so the tests
//! run with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! to avoid process-wide CWD races).

use std::sync::Mutex;

use llg::core::compile;
use llg::sim::{self, opt::OptConfig};

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Compile, codegen, build and run `sv` (top module `tb`); returns the exact
/// stdout plus the non-fatal codegen warnings.
fn run_design(sv: &str, tag: &str) -> Result<(String, Vec<String>), String> {
    run_design_with_opts(sv, tag, &OptConfig::default())
}

fn run_design_with_opts(
    sv: &str,
    tag: &str,
    options: &OptConfig,
) -> Result<(String, Vec<String>), String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join(format!("{tag}.sv"));
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        let gen = sim::codegen::generate_from_db_with_opts(&db, options)
            .map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        let stdout = sim_harness::run_executable(&exe)?;
        Ok((stdout, gen.warnings))
    })
}

/// The bus design from the feature spec: two child drivers plus a parent
/// driver on one collapsed inout net, traced with `$monitor`.
///
/// The design text is verbatim from the spec; the trace below is the ACTUAL
/// runtime behavior, which differs from the spec's hand-sim in three ways:
///   - registration queues the first report until the settled observation
///     point, so the initial assignments coalesce into one t=0 `zz` line;
///   - the monitor also displays `$time` and `en1`; `en1` changes at t=20,
///     while the display-only `$time` query does not trigger a report at t=70
///     when all signal arguments are unchanged (the equal-driver no-re-fire
///     is covered by `sim_inout_equal_drivers_no_refire`, whose monitor watches
///     only the bus);
///   - 0x0a + 0x0b conflict only in bit 0 (low nibble), so `%h` prints
///     `0x`, not `ax`.
///
/// Hand-simulated trace (spawn order at t=0: combs, links, procs; every
/// signal starts all-X):
///   t=0  pass 1: ca_tb drives slot drv=X; ca_u0/ca_u1 drive their slots X
///        (child-side en/d still X before the input links run); resolved = X.
///        Input links copy en0/d0/en1/d1 into the children.  initial:
///        $monitor registration queues a report, then drv=zz, d0=0a, d1=0a,
///        en0=0, en1=0; #10.  The assignments wake the combs, which settle the slots to
///        zz+zz+zz -> resolved zz; check_monitor -> "0 bus=zz drv=zz en0=0
///        en1=0".
///   t=10 en0=1 -> u0 drives 0a -> "10 bus=0a drv=zz en0=1 en1=0".
///   t=20 en1=1 -> u1 drives 0a; resolved stays 0a (equal drivers), but en1
///        changed -> "20 bus=0a drv=zz en0=1 en1=1".
///   t=30 d1=0b -> 0a+0b: only bit 0 conflicts -> "30 bus=0x drv=zz en0=1
///        en1=1".
///   t=40 d0=f0 -> f0+0b: every nibble mixed -> "40 bus=xx drv=zz en0=1
///        en1=1".
///   t=50 en0=0, en1=0, drv=5a -> children release (zz), parent drives 5a ->
///        "50 bus=5a drv=5a en0=0 en1=0".
///   t=60 drv=xx -> "60 bus=xx drv=xx en0=0 en1=0".
///   t=70 $finish; only the display-only `$time` query changed, so no line.
#[test]
fn sim_inout_bus_resolution() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module ram16 #(parameter W=8)(input wire en, input wire [W-1:0] d, inout wire [W-1:0] bus);
    assign bus = en ? d : 8'hzz;
endmodule
module tb;
    wire [7:0] bus;  reg [7:0] drv, d0, d1;  reg en0, en1;
    assign bus = drv;
    ram16 u0(.en(en0), .d(d0), .bus(bus));
    ram16 u1(.en(en1), .d(d1), .bus(bus));
    initial begin
        $monitor("%0t bus=%h drv=%h en0=%b en1=%b", $time, bus, drv, en0, en1);
        drv=8'hzz; d0=8'h0a; d1=8'h0a; en0=0; en1=0;
        #10 en0=1;
        #10 en1=1;
        #10 d1=8'h0b;
        #10 d0=8'hf0;
        #10 en0=0; en1=0; drv=8'h5a;
        #10 drv=8'hxx;
        #10 $finish;
    end
endmodule
"#;
    let (stdout, _warnings) = run_design(sv, "bus").expect("bus design should run");
    assert_eq!(
        stdout,
        "0 bus=zz drv=zz en0=0 en1=0\n\
         10000 bus=0a drv=zz en0=1 en1=0\n\
         20000 bus=0a drv=zz en0=1 en1=1\n\
         30000 bus=0x drv=zz en0=1 en1=1\n\
         40000 bus=xx drv=zz en0=1 en1=1\n\
         50000 bus=5a drv=5a en0=0 en1=0\n\
         60000 bus=xx drv=xx en0=0 en1=0\n"
    );
}

/// The equal-driver no-re-fire property the bus design is meant to show: two
/// drivers holding the SAME value must not re-fire the net's readers.  The
/// monitor watches only the bus (no `$time`/`en1`, which would re-fire every
/// step), and the initial block assigns + `#0` before registering it so the
/// registration print sees the settled all-Z bus.
///
/// Hand-simulated trace (combs then links then procs at t=0; `#0` lets the
/// combs settle before the monitor registers):
///   t=0  pass 1: combs evaluate with the child-side inputs still X ->
///        resolved X; links copy en0/d0/en1/d1 into the children.  initial:
///        drv=zz, d0=0a, d1=0a, en0=0, en1=0 -> wakes ca_tb + links; #0.
///        Re-run: slots settle to zz+zz+zz -> resolved zz.  #0 resume:
///        $monitor registration -> "bus=zz" (snapshot zz).  #10.
///   t=10 en0=1 -> u0 drives 0a -> "bus=0a".
///   t=20 en1=1 -> u1 drives 0a; resolved unchanged (equal drivers) -> NO
///        line.
///   t=30 d1=0b -> 0a+0b: bit 0 conflicts -> "bus=0x".
///   t=40 d0=f0 -> f0+0b -> "bus=xx".
///   t=50 en0=0, en1=0, drv=5a -> children release, parent drives 5a ->
///        "bus=5a".
///   t=60 drv=xx -> "bus=xx".
///   t=70 $finish; nothing changed -> no line.
#[test]
fn sim_inout_equal_drivers_no_refire() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module ram16 #(parameter W=8)(input wire en, input wire [W-1:0] d, inout wire [W-1:0] bus);
    assign bus = en ? d : 8'hzz;
endmodule
module tb;
    wire [7:0] bus;  reg [7:0] drv, d0, d1;  reg en0, en1;
    assign bus = drv;
    ram16 u0(.en(en0), .d(d0), .bus(bus));
    ram16 u1(.en(en1), .d(d1), .bus(bus));
    initial begin
        drv=8'hzz; d0=8'h0a; d1=8'h0a; en0=0; en1=0;
        #0;
        $monitor("bus=%h", bus);
        #10 en0=1;
        #10 en1=1;
        #10 d1=8'h0b;
        #10 d0=8'hf0;
        #10 en0=0; en1=0; drv=8'h5a;
        #10 drv=8'hxx;
        #10 $finish;
    end
endmodule
"#;
    let (stdout, _warnings) = run_design(sv, "norefire").expect("no-re-fire design should run");
    assert_eq!(stdout, "bus=zz\nbus=0a\nbus=0x\nbus=xx\nbus=5a\nbus=xx\n");
}

/// A selected continuous driver on one side of a collapsed inout group gets
/// its own Z-backed structural slot, so unrelated bits remain undriven.
#[test]
fn sim_inout_selected_member_write_resolves() {
    sim_cli::run_case(
        "net_resolution",
        "selected_member_inout",
        "bus=1/zzzzzzz\nbus=0/zzzzzzz\n",
        "",
        &[],
    );
}

/// A delayed selected driver keeps its own Z-backed contribution while the
/// collapsed inout net continues to resolve the child's input dependency.
#[test]
fn sim_inout_selected_member_delayed_driver_resolves() {
    sim_cli::run_case(
        "net_resolution",
        "selected_member_delayed",
        "PASS selected_member_delayed\n",
        "",
        &[],
    );
}
