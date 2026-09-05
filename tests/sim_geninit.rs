//! End-to-end simulator tests for generate-block processes and scalar
//! declaration initializers: Surelog compile → codegen → CMake build → run.
//!
//! Surelog writes `slpp_all/` into the process working directory, so each
//! test runs with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, like the other Surelog integration tests).

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile and run one `tb` design through the shared simulator harness.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// A generate loop with one always process per iteration.  Each iteration's
/// process must be emitted (previously skipped with a warning) with the
/// genvar `i` inlined to the per-scope parameter value, writing its own array
/// element on `posedge clk`.
///
/// Hand-simulation:
///
///   t=0   spawn order: initial (instance process), then the four gen-scope
///        always processes.  clk=X, arr[*]=X.
///        initial: clk=0 (X->0), then #5.
///        each gen process registers its `posedge clk` waiter with
///        last-seen clk=0.
///   t=5   initial wakes: clk=1 (0->1 POSEDGE) -> the four gen processes
///        wake and record arr[i] <= i (i inlined per scope: 0,1,2,3); NBA
///        region commits arr[0]=0, arr[1]=1, arr[2]=2, arr[3]=3.  initial
///        then #5 -> t=10.
///   t=10  initial: clk=0 (negedge, not watched); #5 -> t=15.
///   t=15  initial: clk=1 posedge -> gen processes re-record arr[i] <= i
///        (same values); initial #1 -> t=16.
///   t=16  initial: $display("arr=0 1 2 3"); $finish.
///
/// Expected stdout (exactly):
///   arr=0 1 2 3
#[test]
fn sim_gen_loop_processes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg [7:0] arr [0:3];
    genvar i;
    for (i = 0; i < 4; i = i + 1) begin : g
        always @(posedge clk) arr[i] <= i;
    end
    initial begin
        clk = 0;
        #5 clk = 1;
        #5 clk = 0;
        #5 clk = 1;
        #1 $display("arr=%d %d %d %d", arr[0], arr[1], arr[2], arr[3]);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "genloop").expect("simulation should run");
    assert_eq!(stdout, "arr=0 1 2 3\n");
}

/// A conditional generate (`if (MODE == 0) … else …`) with a process in each
/// branch.  With MODE=1 only the else-branch process must exist and drive `x`
/// to 0 on each posedge; `x` must stay X before the first posedge (nothing
/// else drives it).
///
/// Hand-simulation:
///
///   t=0   spawn order: initial, then the else-branch gen process (the
///        if-branch was elaborated away).  clk=X, x=X.
///        initial: clk=0, then #1.
///        the gen process registers its `posedge clk` waiter (last-seen
///        clk=0).
///   t=1   initial: $display("t=1 x=x") (x still X); #4 -> t=5.
///   t=5   initial: clk=1 posedge -> gen process wakes, records x <= 0; NBA
///        commits x=0.  initial #1 -> t=6.
///   t=6   initial: $display("t=6 x=0"); #5 -> t=11.
///   t=11  initial: clk=0; #5 -> t=16.
///   t=16  initial: clk=1 posedge -> x <= 0 again; initial #1 -> t=17.
///   t=17  initial: $display("t=17 x=0"); $finish.
///
/// Expected stdout (exactly):
///   t=1 x=x
///   t=6 x=0
///   t=17 x=0
#[test]
fn sim_cond_generate_processes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg x;
    localparam MODE = 1;
    generate
        if (MODE == 0) begin : a
            always @(posedge clk) x <= 1;
        end else begin : b
            always @(posedge clk) x <= 0;
        end
    endgenerate
    initial begin
        clk = 0;
        #1 $display("t=%0t x=%b", $time, x);
        #4 clk = 1;
        #1 $display("t=%0t x=%b", $time, x);
        #5 clk = 0;
        #5 clk = 1;
        #1 $display("t=%0t x=%b", $time, x);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "gencond").expect("simulation should run");
    assert_eq!(stdout, "t=1 x=x\nt=6 x=0\nt=17 x=0\n");
}

/// Scalar declaration initializers (`wire w = 1'b1;`, `reg [3:0] r = 4'ha;`,
/// `reg v = 1'b0;` — previously rejected) must be applied in `main()` before
/// any process runs, and a process writing the signal at t=0 must override
/// the initializer.
///
/// Hand-simulation:
///
///   t=0   main() fills the declaration initializers (w=1, r=4'ha, v=0)
///        before spawning any process.  Spawn: initial only.
///        initial: $display("w=1 r=a v=0"); v = 1'b1 (blocking write,
///        overriding the initializer); $display("w=1 r=a v=1"); $finish.
///
/// Expected stdout (exactly):
///   w=1 r=a v=0
///   w=1 r=a v=1
#[test]
fn sim_scalar_decl_inits() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    wire w = 1'b1;
    reg [3:0] r = 4'ha;
    reg v = 1'b0;
    initial begin
        $display("w=%b r=%h v=%b", w, r, v);
        v = 1'b1;
        $display("w=%b r=%h v=%b", w, r, v);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "declinit").expect("simulation should run");
    assert_eq!(stdout, "w=1 r=a v=0\nw=1 r=a v=1\n");
}

/// A scalar variable declaration initializer whose RHS is not a constant
/// expression must still be rejected. Unlike a true-net declaration
/// assignment, this is initialization rather than a continuous driver.
#[test]
fn sim_variable_decl_init_nonconst_rejected() {
    let sv = r#"// llg-test-fixture: tests/sim_geninit.rs/nonconst.sv
module tb;
    reg a;
    reg w = a;
    initial $finish;
endmodule
"#;

    let result = sim_harness::with_surelog_temp_cwd("declnc", |dir| {
        let source = dir.join("nonconst.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        sim::codegen::generate(design)
            .map(|_| ())
            .map_err(|error| error.to_string())
    });

    let err = result.expect_err("codegen must reject non-constant variable initializers");
    assert!(
        err.contains("declaration initializer"),
        "unexpected error: {err}"
    );
}
