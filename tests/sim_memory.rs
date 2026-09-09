//! End-to-end simulator tests for unpacked arrays and memories: Slang
//! compile → codegen → CMake build → run.
//!
//! Covers: single-dimension RAM with non-blocking element writes (including
//! the read-before-write hazard of a same-block `rdata <= mem[addr]`),
//! out-of-range index semantics (read → X, write → no-op), multi-dimensional
//! arrays (row-major linearization, leftmost dimension slowest), wide index
//! values that must not alias through their low 64 bits, and
//! declaration initializers (`'{…}` patterns on variables and nets).
//!
//! These tests temporarily change the process working directory, so the test
//! runs with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! to avoid process-wide CWD races).

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile `sv`, codegen, compile the model + runtime, run it, and return the
/// stdout.  Fails the test on any compile/codegen/cmake/run error.
fn run_sim(name: &str, sv: &str) -> String {
    sim_harness::run_sim(sv, "tb", name).expect("simulation should run")
}

/// (a) A byte-wide RAM driven by `always @(posedge clk)` with non-blocking
/// element writes and a registered read.  The testbench writes `mem[5]` and
/// `mem[9]` and reads them back on later posedges.
///
/// Hand-simulation (all signals X at t=0; spawn order: always@(posedge clk),
/// always#5, initial):
///
///   t=0  initial: clk=0, we=0, addr=0, wdata=0; #3.
///   t=3  initial: we=1, addr=5, wdata=a5; #10.
///   t=5  clk 0->1 posedge: we==1 -> mem[5]<=a5 (NBA); rdata<=mem[5] (still X,
///        read-before-write); NBA region commits mem[5]=a5, rdata=X.
///   t=10 clk 1->0 (negedge, not watched).
///   t=13 initial: we=0, addr=5; #10.
///   t=15 clk 0->1 posedge: we==0 -> no write; rdata<=mem[5]=a5 -> rdata=a5.
///   t=20 clk 1->0.
///   t=23 initial: $display("rdata=a5"); #10.
///   t=25 clk 0->1 posedge: rdata<=mem[5]=a5 (unchanged).
///   t=30 clk 1->0.
///   t=33 initial: we=1, addr=9, wdata=5a; #10.
///   t=35 clk 0->1 posedge: we==1 -> mem[9]<=5a; rdata<=mem[9]=X.
///   t=40 clk 1->0.
///   t=43 initial: we=0, addr=9; #10.
///   t=45 clk 0->1 posedge: rdata<=mem[9]=5a -> rdata=5a.
///   t=50 clk 1->0.
///   t=53 initial: $display("rdata=5a"); $finish.
///
/// Expected stdout (exactly):
///   rdata=a5
///   rdata=5a
#[test]
fn sim_mem_ram_nba() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg we;
    reg [7:0] addr;
    reg [7:0] wdata;
    reg [7:0] rdata;
    reg [7:0] mem [0:255];

    always @(posedge clk) begin
        if (we) mem[addr] <= wdata;
        rdata <= mem[addr];
    end

    always #5 clk = ~clk;

    initial begin
        clk = 0;
        we = 0;
        addr = 0;
        wdata = 8'h0;
        #3 we = 1; addr = 8'd5; wdata = 8'ha5;
        #10 we = 0; addr = 8'd5;
        #10 $display("rdata=%h", rdata);
        #10 we = 1; addr = 8'd9; wdata = 8'h5a;
        #10 we = 0; addr = 8'd9;
        #10 $display("rdata=%h", rdata);
        $finish;
    end
endmodule
"#;
    assert_eq!(run_sim("ram", sv), "rdata=a5\nrdata=5a\n");
}

/// (b) Read-before-write hazard: `rdata <= mem[addr]` in the same always block
/// as `mem[addr] <= wdata` reads the OLD element value when `we` is set (NBA
/// semantics — the write commits after the read was sampled).
///
/// Hand-simulation:
///
///   t=0  initial: mem[0]=55, mem[1]=66, clk=0, we=0, addr=0, wdata=0; #1.
///   t=1  initial: we=1, addr=0, wdata=77; #9.
///   t=5  clk 0->1 posedge: we==1 -> mem[0]<=77; rdata<=mem[0] — reads the OLD
///        value 55 (read-before-write); commit mem[0]=77, rdata=55.
///   t=10 clk 1->0; initial: we=0, addr=1; #3.
///   t=13 initial: $display("t=13 rdata=55") — rdata still holds the
///        pre-write value sampled at t=5.
///   t=15 clk 0->1 posedge: we==0; rdata<=mem[1]=66 -> rdata=66.
///   t=20 initial: $display("t=20 rdata=66"); $finish.
///
/// Expected stdout (exactly):
///   t=13 rdata=55
///   t=20 rdata=66
#[test]
fn sim_mem_read_before_write() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk, we;
    reg [7:0] addr, wdata, rdata;
    reg [7:0] mem [0:3];

    always @(posedge clk) begin
        if (we) mem[addr] <= wdata;
        rdata <= mem[addr];
    end
    always #5 clk = ~clk;

    initial begin
        mem[0] = 8'h55;
        mem[1] = 8'h66;
        clk = 0;
        we = 0;
        addr = 0;
        wdata = 0;
        #1 we = 1; addr = 0; wdata = 8'h77;
        #9 we = 0; addr = 1;
        #3 $display("t=%0d rdata=%h", $time, rdata);
        #7 $display("t=%0d rdata=%h", $time, rdata);
        $finish;
    end
endmodule
"#;
    assert_eq!(run_sim("hazard", sv), "t=13 rdata=55\nt=20 rdata=66\n");
}

/// (c) Out-of-range index semantics: reading `mem[300]` (past `[0:255]`)
/// yields X, writing it is a no-op that neither crashes nor corrupts the
/// memory.
#[test]
fn sim_mem_out_of_range() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] mem [0:255];
    reg [7:0] r;
    initial begin
        mem[10] = 8'haa;
        r = mem[300];
        $display("oor=%h", r);
        mem[300] = 8'h5;
        $display("mem10=%h", mem[10]);
        $finish;
    end
endmodule
"#;
    assert_eq!(run_sim("oor", sv), "oor=xx\nmem10=aa\n");
}

/// Wide array indices whose values do not fit in `int64_t` are out of range;
/// they must not alias an element through their low 64 bits.  A signed
/// 128-bit index is positive here but still exceeds `int64_t::MAX`.
#[test]
fn sim_mem_wide_index_does_not_alias_low64() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] mem [0:3];
    reg [127:0] unsigned_idx;
    reg signed [127:0] signed_idx;
    reg [7:0] read_unsigned, read_signed, read_valid;

    initial begin
        mem[0] = 8'h10;
        mem[1] = 8'h21;
        mem[2] = 8'h32;
        mem[3] = 8'h43;
        unsigned_idx = 128'h1_0000_0000_0000_0001;
        signed_idx = 128'sh1_0000_0000_0000_0001;

        read_unsigned = mem[unsigned_idx];
        read_signed = mem[signed_idx];
        read_valid = mem[1];
        $display("read_unsigned=%h read_signed=%h read_valid=%h",
                 read_unsigned, read_signed, read_valid);

        mem[unsigned_idx] = 8'he1;
        mem[signed_idx] = 8'he2;
        $display("mem0=%h mem1=%h mem2=%h mem3=%h",
                 mem[0], mem[1], mem[2], mem[3]);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("wide_index", sv),
        "read_unsigned=xx read_signed=xx read_valid=21\nmem0=10 mem1=21 mem2=32 mem3=43\n"
    );
}

/// (d) Multi-dimensional array `logic [3:0] a [0:1][0:3]` — element accesses
/// linearize row-major with the leftmost dimension slowest (Verilog
/// convention): linear = (i0 - left0) * size1 + (i1 - left1).
#[test]
fn sim_mem_multi_dim() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [3:0] a [0:1][0:3];
    initial begin
        a[0][1] = 4'h3;
        a[1][3] = 4'hc;
        a[0][0] = 4'h1;
        a[1][2] = 4'h8;
        $display("a00=%h a01=%h a12=%h a13=%h", a[0][0], a[0][1], a[1][2], a[1][3]);
        $finish;
    end
endmodule
"#;
    assert_eq!(run_sim("multidim", sv), "a00=1 a01=3 a12=8 a13=c\n");
}

/// (e) Declaration initializers (`= '{…}`) for both `reg` and `logic` arrays.
/// The pattern is applied before any process runs; later element writes behave
/// normally.
#[test]
fn sim_mem_decl_initializer() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] mem [0:3] = '{8'h1, 8'h2, 8'h3, 8'h4};
    logic [7:0] lm [0:3] = '{8'ha, 8'hb, 8'hc, 8'hd};
    initial begin
        $display("mem0=%h mem1=%h mem2=%h mem3=%h", mem[0], mem[1], mem[2], mem[3]);
        $display("lm0=%h lm3=%h", lm[0], lm[3]);
        mem[2] = 8'hf0;
        $display("after mem2=%h mem3=%h", mem[2], mem[3]);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("init", sv),
        "mem0=01 mem1=02 mem2=03 mem3=04\nlm0=0a lm3=0d\nafter mem2=f0 mem3=04\n"
    );
}

/// A single unpacked dimension expression is an implicit `[0:size-1]` range.
#[test]
fn sim_mem_implicit_size_is_zero_based() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] mem [8];
    initial begin
        mem[0] = 8'h11;
        mem[7] = 8'h77;
        $display("m0=%h m7=%h", mem[0], mem[7]);
        $finish;
    end
endmodule
"#;
    assert_eq!(run_sim("implicit_size", sv), "m0=11 m7=77\n");
}

/// Foreach forms with an omitted dimension index remain an explicit codegen
/// boundary rather than silently iterating the wrong shape.
#[test]
fn sim_mem_foreach_rejected() {
    let sv = r#"module tb;
    logic [7:0] mem [0:1][0:1];
    initial begin
        foreach (mem[i,]) mem[i][0] = i;
    end
endmodule
"#;
    let result = sim_harness::with_frontend_temp_cwd("mem_foreach_rej", |dir| {
        let source = dir.join("foreach_rej.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        match sim::codegen::generate(&db) {
            Ok(_) => Err("codegen unexpectedly succeeded".to_string()),
            Err(e) => Ok(e.to_string()),
        }
    });

    let err = result.expect("codegen should fail");
    assert!(
        err.contains("requires one explicit index variable per dimension"),
        "unexpected error: {err}"
    );
}
