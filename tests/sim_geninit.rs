//! End-to-end simulator tests for generate-block processes and scalar
//! declaration initializers: Slang compile → codegen → CMake build → run.
//!
//! These tests temporarily change the process working directory, so each
//! test runs with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

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
    assert_eq!(stdout, "t=1000 x=x\nt=6000 x=0\nt=17000 x=0\n");
}

/// Nested direct conditional scopes retain all executable content: the child
/// instance connects through its ports, the continuous assignment propagates
/// the process result, and the selected process observes the connected value.
#[test]
fn sim_nested_cond_generate_retains_process_driver_and_child_links() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"// llg-test-fixture: tests/sim_geninit.rs/nested_conditional.sv
module leaf(input wire source, output wire linked);
    assign linked = source;
endmodule

module tb;
    reg clk;
    reg source;
    reg sampled;
    wire observed;
    localparam OUTER = 1;
    localparam INNER = 1;

    generate
        if (OUTER) begin : outer
            if (INNER) begin : inner
                wire linked;
                leaf child(.source(source), .linked(linked));
                always @(posedge clk) sampled <= linked;
                assign observed = sampled;
            end
        end
    endgenerate

    initial begin
        clk = 0;
        source = 0;
        #1 source = 1;
        #4 clk = 1;
        #1 $display("observed=%0b", observed);
        $finish;
    end
endmodule
"#;
    let expected = "observed=1\n";

    sim_harness::with_frontend_temp_cwd("nested-cond-generate", |dir| {
        let source = dir.join("nested_conditional.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database = llg::core::db::Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("database: {error}"))?;

        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let generated = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} codegen: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", generated.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} cmake: {error}"))?;
            let stdout = sim_harness::run_executable(&executable)?;
            if stdout != expected {
                return Err(format!("{variant}: expected {expected:?}, got {stdout:?}"));
            }
        }
        Ok(())
    })
    .expect("nested direct conditional generate scopes must execute");
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

/// A scalar variable declaration initializer may read a runtime signal. It is
/// evaluated once before ordinary SystemVerilog processes, unlike a true-net
/// declaration assignment, which remains a continuous driver.
#[test]
fn sim_variable_decl_init_nonconst_runs_once() {
    let sv = r#"// llg-test-fixture: tests/sim_geninit.rs/nonconst.sv
module tb;
    reg a;
    reg w = a;
    initial begin
        a = 1'b1;
        $display("a=%b w=%b", a, w);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "declnonconst").expect("simulation should run");
    assert_eq!(stdout, "a=1 w=x\n");
}

/// A legal `defparam` (IEEE 1364-2001 §12.2.1) overrides one elaborated
/// instance's parameter; an un-overridden sibling keeps the default, so the
/// two children produce different widths. The `defparam` declaration itself
/// must be consumed at elaboration and must not become an executable node.
#[test]
fn sim_defparam_re_elaborates_only_the_selected_instance() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module defparam_child #(parameter W = 4) (output [W-1:0] o);
    assign o = {W{1'b1}};
endmodule
module tb;
    wire [3:0] narrow;
    wire [7:0] wide;
    defparam_child c_narrow (narrow);
    defparam_child c_wide (wide);
    defparam c_wide.W = 8;
    initial begin
        $display("narrow=%b wide=%b", narrow, wide);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "defparam").expect("legal defparam must re-elaborate");
    assert_eq!(stdout, "narrow=1111 wide=11111111\n");
}

/// A module instance array (IEEE 1800-2009 §23.8) elaborates each element as a
/// distinct instance. Before element naming was recovered, lowering rejected
/// the design as an unnamed child instance.
#[test]
fn sim_module_instance_array_elaborates_each_element() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module array_leaf (input [3:0] a, output [3:0] y);
    assign y = ~a;
endmodule
module tb;
    wire [3:0] a0 = 4'h0;
    wire [3:0] a1 = 4'h1;
    wire [3:0] y0;
    wire [3:0] y1;
    array_leaf u[1:0] (.a({a1, a0}), .y({y1, y0}));
    initial begin
        #1 $display("y0=%h y1=%h", y0, y1);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "modarray").expect("instance array must elaborate");
    assert_eq!(stdout, "y0=f y1=e\n");
}

/// IEEE 1800-2009 §5.6 / IEEE 1364-2001 §2.7: escaped identifiers terminate at
/// whitespace and may contain characters (`a.b`, `a-b`) that collide with the
/// punctuation-encoded forms of other source names. The generated C must keep
/// each declaration distinct and load the intended value into each.
#[test]
fn sim_escaped_identifiers_roundtrip_without_c_name_collisions() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg \a.b  = 1'b1;
    reg a_b    = 1'b0;
    reg \a-b   = 1'b1;
    initial begin
        $display("dot=%0b under=%0b dash=%0b", \a.b , a_b, \a-b );
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "escapedident").expect("escaped identifiers must emit safely");
    assert_eq!(stdout, "dot=1 under=0 dash=1\n");
}
