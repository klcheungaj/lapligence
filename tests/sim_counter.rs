//! End-to-end simulator test: Surelog compile → codegen → CMake build → run.
//!
//! Also compiles and runs the C runtime self-test (`llg_rt_selftest.c`).
//!
//! Surelog writes `slpp_all/` into the process working directory, so the test
//! runs with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! like the other Surelog integration tests).

use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// The counter design.  `rst_n` is driven low at t=1 (after a blocking `clk=0;
/// rst_n=1` at t=0) so the reset edge is deterministic: the always block sees
/// a clean `!rst_n` (no X) when it wakes.
const COUNTER_SV: &str = r#"module counter #(parameter WIDTH = 8, parameter [3:0] INIT = 4'h5) (
    input  logic clk, input logic rst_n,
    output logic [WIDTH-1:0] count, output logic done
);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) count <= INIT;
        else count <= count + 1;
    end
    assign done = (count == 8'hff);
endmodule

module tb;
    reg clk; reg rst_n; wire [7:0] count; wire done;
    counter #(.WIDTH(8), .INIT(4'h5)) u(.clk(clk), .rst_n(rst_n), .count(count), .done(done));
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #30 $display("count=%0d done=%b", count, done);
        #10 $display("count=%0d done=%b", count, done);
        $finish;
    end
endmodule
"#;

// Hand-simulation of the design (all signals are the child-instance values;
// tb-side nets mirror them through the port link processes):
//
//   t=0  spawn order: comb(done) -> links -> always@#5 -> always@(edges) -> initial
//        clk=X rst_n=X count=X done=X.
//        initial runs: clk=0 (X->0), rst_n=1 (X->1; posedge of rst_n is NOT
//        watched), then #1.
//        always@(edges) registers its waiters with last-seen clk=0, rst_n=1.
//   t=1  initial wakes: rst_n=0 -> 1->0 NEGEDGE -> counter wakes; !rst_n==1 ->
//        count<=INIT(4'h5) recorded; NBA region commits count=5; done comb
//        recomputes done = (5==255) = 0.
//   t=4  initial wakes: rst_n=1 -> 0->1 posedge (not watched); #30 -> t=34.
//   t=5  always#5: clk=~0=1 -> 0->1 POSEDGE -> counter wakes; rst_n==1 ->
//        count<=5+1=6 -> commit -> done=0.
//   t=10 clk=0 (negedge, not watched).
//   t=15 clk=1 posedge -> count<=7.
//   t=20 clk=0.
//   t=25 clk=1 posedge -> count<=8.
//   t=30 clk=0.
//   t=34 initial: $display("count=8 done=0"); #10 -> t=44.
//   t=35 clk=1 posedge -> count<=9.
//   t=44 initial: $display("count=9 done=0"); $finish -> scheduler stops
//        (the always#5 clock keeps scheduling t=45+, but $finish ends the run).
//
// Expected stdout (exactly):
//   count=8 done=0
//   count=9 done=0

#[test]
fn sim_counter_end_to_end() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let stdout = run_sim(COUNTER_SV, "counter").expect("simulation should run");
    assert_eq!(stdout, "count=8 done=0\ncount=9 done=0\n");
}

/// Compile and run the C runtime self-test (sv4 math vectors + scheduler
/// checks: delay ordering, NBA visibility, ping-pong).
#[test]
fn sim_rt_selftest() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("runtime-selftest").expect("create temp dir");
    let exe = sim::build::build_model_cmake(
        dir.path(),
        &[("llg_rt_selftest.c", sim::rt::selftest_source())],
    )
    .expect("selftest should compile");
    sim_harness::run_executable(&exe).expect("selftest should run");
}

/// always_comb (no explicit event control) must evaluate once at t=0 and then
/// re-run only when a READ signal changes — the LHS must not be in the
/// sensitivity set (no self-wake), and %t must consume its $time argument.
#[test]
fn sim_always_comb_and_display_t() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a;
    reg [3:0] out;
    always_comb begin
        out = a ? 4'd1 : 4'd2;
    end
    initial begin
        a = 0;
        #1 $display("t=%0t out=%0d", $time, out);
        a = 1;
        #1 $display("t=%0t out=%0d", $time, out);
        a = 0;
        #1 $display("t=%0t out=%0d", $time, out);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "comb").expect("simulation should run");
    // t=1: out still 2 (a=1 assigned after the display); t=2: out=1; t=3: out=2.
    assert_eq!(stdout, "t=1 out=2\nt=2 out=1\nt=3 out=2\n");
}

/// A packed vector wider than the former 1024-bit implementation ceiling must
/// preserve low, middle, and high bits through context-sized arithmetic.
#[test]
fn sim_wide_signal_2048() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/wide_datatype_regressions/wide_signal_2048.sv");
    let source = std::fs::read_to_string(&fixture).expect("read wide-signal fixture");
    let stdout = run_sim(&source, "wide-signal-2048").expect("simulation should run");
    assert_eq!(stdout, "PASS wide_signal_2048\n");
}

/// A 128-bit counter driven on `posedge clk`: wide signals, wide constants
/// (`128'd0`) and wide arithmetic (`count + 1`) must run on the limb-based
/// runtime, with `%0d` printing the exact decimal value.
#[test]
fn sim_wide_counter_128() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic clk;
    logic rst_n;
    logic [127:0] count;
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) count <= 128'd0;
        else count <= count + 1;
    end
    always #5 clk = ~clk;
    initial begin
        clk = 0;
        rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #30 $display("count=%0d", count);
        #10 $display("count=%0d", count);
        $finish;
    end
endmodule
"#;
    // Hand-simulation (mirrors sim_counter_end_to_end with INIT = 0):
    //
    //   t=0  spawn order: always@(edges) -> always#5 -> initial (db children
    //        order).  clk=X rst_n=X count=X.
    //        always@(edges) registers waiters with last-seen clk=X, rst_n=X;
    //        always#5 waits #5.
    //        initial: clk=0 (X->0, not posedge), rst_n=1 (X->1, not negedge;
    //        both miss the always's watched edges), then #1.
    //   t=1  initial wakes: rst_n=0 -> 1->0 NEGEDGE -> always wakes; !rst_n ->
    //        count<=128'd0 recorded; NBA commits count=0.
    //   t=4  initial wakes: rst_n=1 -> 0->1 posedge (not watched); #30 -> t=34.
    //   t=5  always#5: clk=~0=1 -> 0->1 POSEDGE -> count<=0+1=1 -> count=1.
    //   t=10 clk=0 (negedge, not watched).
    //   t=15 clk=1 posedge -> count<=2.
    //   t=20 clk=0.
    //   t=25 clk=1 posedge -> count<=3.
    //   t=30 clk=0.
    //   t=34 initial: $display("count=3"); #10 -> t=44.
    //   t=35 clk=1 posedge -> count<=4.
    //   t=44 initial: $display("count=4"); $finish.
    //
    // Expected stdout (exactly):
    //   count=3
    //   count=4

    let stdout = run_sim(sv, "wide-counter").expect("simulation should run");
    assert_eq!(stdout, "count=3\ncount=4\n");
}

/// A 128-bit concat of two 64-bit values: `w = {a, b}` exercises the
/// limb-wide `sv4_concat`; `%0d` prints the exact 128-bit decimal.
#[test]
fn sim_wide_concat() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [63:0] a, b;
    wire [127:0] w;
    assign w = {a, b};
    initial begin
        a = 64'd1;
        b = 64'd18446744073709551615;
        #1 $display("w=%0d", w);
        $finish;
    end
endmodule
"#;
    // Hand-simulation: w = {1, 2^64-1} = 2^65 - 1 = 36893488147419103231.
    //
    //   t=0  spawn order: comb(w) -> initial.
    //        comb evaluates w = {X, X} = all-X, then waits on {a, b}.
    //        initial: a=1 (X->1 wakes comb, queued), b=2^64-1 (queued again),
    //        then #1 suspends; comb runs: w = {1, 2^64-1}.
    //   t=1  initial: $display("w=36893488147419103231"); $finish.
    //
    // Expected stdout (exactly):
    //   w=36893488147419103231

    let stdout = run_sim(sv, "wide-concat").expect("simulation should run");
    assert_eq!(stdout, "w=36893488147419103231\n");
}

/// `time` variables must be at least 64 bits (LRM 1364-1995 §3.10.2 /
/// 1364-2001 §3.11.2).  Regression: the db mapped `vpiTimeTypespec` to width
/// 32, so values above 2^32 were silently truncated on assignment.
#[test]
fn sim_time_var_is_64bit() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    time t;
    initial begin
        t = 64'd10_000_000_000;
        $display("t=%0d", t);
        $finish;
    end
endmodule
"#;
    // Expected stdout (exactly): the full 64-bit value survives storage.
    //   t=10000000000

    let stdout = run_sim(sv, "time64").expect("simulation should run");
    assert_eq!(stdout, "t=10000000000\n");
}

/// Static casts `int'(e)`, `signed'(e)`, `unsigned'(e)`, `n'(e)` (§1800-2009
/// 6.24.1) lower to value-preserving conversions: widening extends by the
/// SOURCE's signedness (an unsigned source zero-extends even into a signed
/// target, §6.24.1 "the value that a variable of the casting type would hold
/// after being assigned the expression" + §10.7), narrowing truncates, and a
/// sign-only cast retags at unchanged width.  Assignment contexts pad by the
/// RHS's own signedness too (§10.7 / §11.8.3 / 1364-2001 §4.5.3): an
/// unsigned RHS widens into a wider signed LHS by zero-extension and a
/// signed RHS into a wider unsigned LHS by sign-extension.
#[test]
fn sim_static_casts() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [7:0] b;           // unsigned 8-bit source
    logic [7:0] bh;          // unsigned source with the MSB set
    logic signed [7:0] c;    // signed 8-bit source
    logic signed [3:0] n;    // narrow signed source (part-select widening)
    logic signed [15:0] s;   // wider signed target
    logic [15:0] u;          // wider unsigned target
    logic [23:0] field;
    logic signed [15:0] ini = 8'hFF;  // declaration initializer assignment
    initial begin
        b = 8'h7F;
        bh = 8'hFF;
        c = -2;
        n = -1;
        // Sign-only casts retag at unchanged width:
        $display("pos=%0d %0d %0d", int'(b), signed'(b), unsigned'(b));
        $display("neg=%0d %0d %0d", int'(c), signed'(c), unsigned'(c));
        // Unsigned MSB-set source widened into a wider signed target must
        // ZERO-extend (was -1 before the §6.24.1 fix):
        $display("u2s=%0d", int'(bh));
        // Nested sign+size casts stay value-stable.
        $display("s2uw=%0d", unsigned'(32'(c)));
        // Assignment padding follows the RHS signedness (§10.7/§11.8.3):
        s = bh;
        $display("a_u2s_var=%0d", s);
        s = 8'hFF;
        $display("a_u2s_lit=%0d", s);
        u = c;
        $display("a_s2u_var=%0d", u);
        field[15:8] = n;
        $display("a_sel=%0h", field[15:8]);
        // Declaration initializer follows the same §10.7 padding:
        $display("ini=%0d", ini);
        $finish;
    end
endmodule
"#;
    // Hand-simulation (LRM 1800-2009 §6.24.1, §10.7, §11.8.3):
    //   b = 127 unsigned; bh = 255 unsigned; c = -2 signed (8'hFE);
    //   n = -1 signed (4'hF).
    //   pos: int'(b)=127; signed'(b) retags 8'h7F = 127; unsigned'(b)=127.
    //   neg: int'(c) sign-extends -2; signed'(c)=-2; unsigned'(c) retags
    //   8'hFE = 254.
    //   u2s: int'(bh) zero-extends 255 into the signed int -> 255.
    //   s2uw: the pinned Surelog captures an n'(e) size-cast target as int
    //   (32-bit unsigned), so 32'(c) zero-extends c into 32 bits and
    //   unsigned'() retags at unchanged width -> 4294967294.  (The pure
    //   signed-source-widening-into-unsigned cast quadrant is pinned at the
    //   elab/rt/vector-table layers and by the a_s2u_var assignment below;
    //   the size-cast width capture is a Surelog frontend limitation.)
    //   a_u2s_var/a_u2s_lit: unsigned RHS zero-extends into the wider signed
    //   LHS -> 255 (was -1 before the §10.7 fix).
    //   a_s2u_var: signed RHS sign-extends into the wider unsigned LHS ->
    //   16'hFFFE = 65534.
    //   a_sel: the 4-bit signed -1 sign-extends into the 8-bit select field
    //   -> 8'hff.
    //   ini: the initializer assignment zero-extends unsigned 8'hFF into the
    //   wider signed variable -> 255.
    //
    // Expected stdout (exactly):
    //   pos=127 127 127
    //   neg=-2 -2 254
    //   u2s=255
    //   s2uw=4294967294
    //   a_u2s_var=255
    //   a_u2s_lit=255
    //   a_s2u_var=65534
    //   a_sel=ff
    //   ini=255

    let stdout = run_sim(sv, "casts").expect("simulation should run");
    assert_eq!(
        stdout,
        "pos=127 127 127\nneg=-2 -2 254\nu2s=255\ns2uw=4294967294\n\
         a_u2s_var=255\na_u2s_lit=255\na_s2u_var=65534\na_sel=ff\nini=255\n"
    );
}
