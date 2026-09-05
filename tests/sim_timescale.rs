//! End-to-end simulator tests for timescale-aware delays: `#N` delays scale
//! by the calling module's `timescale unit`, and `$time`/`$stime` return the
//! current time in the calling module's unit.
//!
//! The scheduler runs in design-precision ticks (the finest precision across
//! the design); the codegen scales `#N` up to ticks and `$time` down to the
//! calling module's unit, so observable behavior matches Verilator's
//! per-module timescale practice.  Modules without a `timescale directive
//! default to 1ns/1ps (TIMESCALEMOD behavior), which keeps the pre-timescale
//! test outputs unchanged.

#[path = "support/sim.rs"]
mod sim_harness;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn stime_is_the_32_bit_form_of_time() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(
        "stime",
        &[(
            "tb.sv",
            "`timescale 1ns/1ns\n\
             module tb;\n\
                 initial begin\n\
                     #4294967298;\n\
                     $display(\"time=%0d stime=%0d\", $time, $stime);\n\
                     $finish;\n\
                 end\n\
             endmodule\n",
        )],
    )
    .expect("simulation should run");
    assert_eq!(stdout, "time=4294967298 stime=2\n");
}

/// Compile `sv` in a fresh temp dir, run the simulator, and return its stdout
/// (the hand-simulated trace is documented per test).
fn run_design(name: &str, files: &[(&str, &str)]) -> Result<String, String> {
    sim_harness::with_temp_cwd(name, |dir| {
        let mut paths = Vec::new();
        for (fname, body) in files {
            let p = dir.join(fname);
            std::fs::write(&p, body).expect("write source");
            paths.push(p.to_string_lossy().into_owned());
        }

        // 1. Surelog compile + elaborate.
        let out = compile::compile(&compile::CompileOpts {
            files: paths,
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;

        // 2. Codegen.
        let gen = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;

        // 3. Build model + runtime + libaco with CMake.
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;

        // 4. Run.
        sim_harness::run_executable(&exe)
    })
}

/// (a) Two modules with DIFFERENT timescales: `#5` in a `10ns/1ns` module is
/// 50 ns while `#50` in a `1ns/1ps` module is 50 ns — both wake at the same
/// wall time and each `$time` shows the unit-scaled value.
///
/// Timescales: slow (10ns/1ns → unit 10000 ps, precision 1000 ps), fast
/// (1ns/1ps → unit 1000 ps, precision 1 ps), tb (no directive → default
/// 1ns/1ps).  Design precision = min(1000, 1, 1) = 1 ps → 1 tick = 1 ps.
///
/// Hand-simulation (ticks in design-precision units):
///   t=0  three initial processes spawn (slow's, fast's, then tb's).
///        slow #5  → 5 * 10000 / 1   = 50000 ticks (50 ns)
///        fast #50 → 50 * 1000 / 1   = 50000 ticks (50 ns)
///        tb   #95 → 95 * 1000 / 1   = 95000 ticks (95 ns)
///   t=50 ns (50000 ticks):
///        slow wakes: $time = 50000 * 1 / 10000 = 5   → "slow t=5"
///        fast wakes: $time = 50000 * 1 / 1000  = 50  → "fast t=50"
///   t=95 ns (95000 ticks):
///        tb wakes:   $time = 95000 * 1 / 1000  = 95  → "tb t=95"; $finish.
///
/// Expected stdout (exactly):
///   slow t=5
///   fast t=50
///   tb t=95
#[test]
fn timescale_cross_module_units() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(
        "cross",
        &[
            (
                "slow.sv",
                "`timescale 10ns/1ns\n\
                 module slow;\n\
                     initial begin\n\
                         #5 $display(\"slow t=%0t\", $time);\n\
                         #5 $display(\"slow t=%0t\", $time);\n\
                     end\n\
                 endmodule\n",
            ),
            (
                "fast.sv",
                "`timescale 1ns/1ps\n\
                 module fast;\n\
                     initial begin\n\
                         #50 $display(\"fast t=%0t\", $time);\n\
                         #50 $display(\"fast t=%0t\", $time);\n\
                     end\n\
                 endmodule\n",
            ),
            (
                "tb.sv",
                "module tb;\n\
                     slow u_slow();\n\
                     fast u_fast();\n\
                     initial begin\n\
                         #95 $display(\"tb t=%0t\", $time);\n\
                         $finish;\n\
                     end\n\
                 endmodule\n",
            ),
        ],
    )
    .expect("simulation should run");
    assert_eq!(stdout, "slow t=5\nfast t=50\ntb t=95\n");
}

/// (b) No timescale → default 1ns/1ps (TIMESCALEMOD): `#5` still displays
/// "t=5" and `$time` returns the unit-scaled time, so the pre-timescale test
/// outputs are unchanged.  Also exercises `$printtimescale`, which prints the
/// calling module's unit/precision.
///
/// Timescales: tb (no directive → default unit 1000 ps, precision 1 ps).
/// Design precision = 1 ps → 1 tick = 1 ps.
///
/// Hand-simulation:
///   t=0  always#5 waits 5000 ticks; initial prints the timescale then waits
///        #5 → 5000 ticks.
///   t=5 ns (5000 ticks): initial wakes, $time = 5000 * 1 / 1000 = 5
///        → "t=5"; waits #10 → t=15 ns.
///   t=15 ns (15000 ticks): initial wakes, $time = 15 → "t=15"; $finish.
///
/// Expected stdout (exactly):
///   tb: timescale is 1ns/1ps
///   t=5
///   t=15
#[test]
fn timescale_default_1ns_1ps() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(
        "default",
        &[(
            "tb.sv",
            "module tb;\n\
                 reg x;\n\
                 always #5 x = ~x;\n\
                 initial begin\n\
                     $printtimescale;\n\
                     #5 $display(\"t=%0t\", $time);\n\
                     #10 $display(\"t=%0t\", $time);\n\
                     $finish;\n\
                 end\n\
             endmodule\n",
        )],
    )
    .expect("simulation should run");
    assert_eq!(stdout, "tb: timescale is 1ns/1ps\nt=5\nt=15\n");
}

/// (c) Sub-unit delays: `#3` with `timescale 10ns/1ns` (unit 10000 ps) in a
/// design whose overall precision is 1 ps (from the 1ns/1ps module) jumps
/// 30 ns in one step.
///
/// Timescales: big (10ns/1ns → unit 10000 ps), tiny (1ns/1ps → unit 1000 ps,
/// precision 1 ps), tb (default 1ns/1ps).  Design precision = 1 ps.
///
/// Hand-simulation:
///   t=0  big #3  → 3 * 10000 / 1 = 30000 ticks (30 ns)
///        tiny #30 → 30 * 1000 / 1 = 30000 ticks (30 ns)
///        tb   #40 → 40 * 1000 / 1 = 40000 ticks (40 ns)
///   t=30 ns (30000 ticks):
///        big wakes:  $time = 30000 * 1 / 10000 = 3  → "big t=3"
///        tiny wakes: $time = 30000 * 1 / 1000  = 30 → "tiny t=30"
///   t=40 ns (40000 ticks):
///        tb wakes:   $time = 40000 * 1 / 1000  = 40 → "tb t=40"; $finish.
///
/// Expected stdout (exactly):
///   big t=3
///   tiny t=30
///   tb t=40
#[test]
fn timescale_sub_unit_delay_jumps() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(
        "subunit",
        &[
            (
                "big.sv",
                "`timescale 10ns/1ns\n\
                 module big;\n\
                     initial begin\n\
                         #3 $display(\"big t=%0t\", $time);\n\
                     end\n\
                 endmodule\n",
            ),
            (
                "tiny.sv",
                "`timescale 1ns/1ps\n\
                 module tiny;\n\
                     initial begin\n\
                         #30 $display(\"tiny t=%0t\", $time);\n\
                     end\n\
                 endmodule\n",
            ),
            (
                "tb.sv",
                "module tb;\n\
                     big u_big();\n\
                     tiny u_tiny();\n\
                     initial begin\n\
                         #40 $display(\"tb t=%0t\", $time);\n\
                         $finish;\n\
                     end\n\
                 endmodule\n",
            ),
        ],
    )
    .expect("simulation should run");
    assert_eq!(stdout, "big t=3\ntiny t=30\ntb t=40\n");
}
