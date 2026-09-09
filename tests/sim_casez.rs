//! End-to-end simulator tests for X/Z distinction: casez/casex wildcard
//! matching (LRM 12.5.1), z-literal display, and `===`/`!==` literal
//! comparison of X vs Z bits.
//!
//! Each test: Slang compile → codegen → CMake build → run, asserting the exact
//! stdout (hand-simulated traces in the comments).

#[path = "support/sim.rs"]
mod sim_harness;

fn run_sim(dir_name: &str, sv: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", dir_name)
}

/// casez priority encoder: `?` (z) in a case item is a don't-care, and an x
/// selector bit falls through like any non-matching bit unless a `?` covers
/// it (LRM 12.5.1).
#[test]
fn sim_casez_priority_encoder() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [3:0] sel;
    reg [3:0] o;
    always_comb begin
        casez (sel)
            4'b1???: o = 4'd3;
            4'b01??: o = 4'd2;
            4'b001?: o = 4'd1;
            default: o = 4'd0;
        endcase
    end
    initial begin
        sel = 4'b1000; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b0100; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b0010; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b0001; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b0000; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b1x00; #1 $display("sel=%b o=%0d", sel, o);
        $finish;
    end
endmodule
"#;
    // Hand-simulation:
    //   t=0  spawn order: comb(o) -> initial.  sel=X -> comb: casez(4'bxxxx)
    //        vs 4'b1??? (bit3 item 1 vs sel x -> no), 4'b01?? (no), 4'b001?
    //        (no) -> default -> o=0.  initial sets sel=4'b1000 (wakes comb,
    //        queued), then #1.
    //        comb: 4'b1000 matches 4'b1??? (bits 2..0 are ?) -> o=3.
    //   t=1  $display("sel=1000 o=3"); sel=4'b0100 wakes comb: 4'b01?? bit3=0
    //        vs sel bit3=0, bit2=1 vs 1 -> o=2; then #1.
    //   t=2  $display("sel=0100 o=2"); sel=4'b0010 -> 4'b001? matches -> o=1.
    //   t=3  $display("sel=0010 o=1"); sel=4'b0001: bit1 of 4'b001? is 1 vs
    //        sel bit1=0 -> no; 4'b1???/4'b01?? no -> default -> o=0.
    //   t=4  $display("sel=0001 o=0"); sel=4'b0000 -> default -> o=0.
    //   t=5  $display("sel=0000 o=0"); sel=4'b1x00: bit3=1 matches 4'b1???,
    //        bits 2..0 are ? -> o=3.
    //   t=6  $display("sel=1x00 o=3"); $finish.
    let stdout = run_sim("llg_casez", sv).expect("casez simulation should run");
    assert_eq!(
        stdout,
        "sel=1000 o=3\nsel=0100 o=2\nsel=0010 o=1\nsel=0001 o=0\nsel=0000 o=0\nsel=1x00 o=3\n"
    );
}

/// casex: x in the case item is a don't-care (unlike casez, where item x
/// matches only selector x).
#[test]
fn sim_casex() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [3:0] sel;
    reg [3:0] o;
    always_comb begin
        casex (sel)
            4'b1x0x: o = 4'd1;
            default: o = 4'd0;
        endcase
    end
    initial begin
        sel = 4'b1000; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b1101; #1 $display("sel=%b o=%0d", sel, o);
        sel = 4'b0110; #1 $display("sel=%b o=%0d", sel, o);
        $finish;
    end
endmodule
"#;
    // Hand-simulation:
    //   t=0  comb evaluates casex(4'bxxxx) vs 4'b1x0x: bit3 item 1 vs sel x ->
    //        in casex a known item bit matches a selector x too -> o=1 (item
    //        bits 2/0 are x, don't-care).  initial then drives sel.
    //   t=1  sel=4'b1000: bit3 1v1, bit1 0v0, bits 2/0 x -> o=1.
    //   t=2  sel=4'b1101: bit3 1v1, bit1 0v0 -> o=1.
    //   t=3  sel=4'b0110: bit3 0 vs item 1 (opposite known) -> default -> o=0.
    //   t=4  $finish.
    let stdout = run_sim("llg_casex", sv).expect("casex simulation should run");
    assert_eq!(stdout, "sel=1000 o=1\nsel=1101 o=1\nsel=0110 o=0\n");
}

/// A default item before an explicit item remains a fallback: the later
/// explicit item must win when it matches, and the default must run otherwise.
#[test]
fn sim_case_default_first() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [1:0] sel;
    reg [1:0] o;
    always_comb begin
        case (sel)
            default: o = 2'd0;
            2'd1: o = 2'd1;
        endcase
    end
    initial begin
        sel = 2'd1; #1 $display("sel=%0d o=%0d", sel, o);
        sel = 2'd2; #1 $display("sel=%0d o=%0d", sel, o);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim("llg_case_default_first", sv).expect("simulation should run");
    assert_eq!(stdout, "sel=1 o=1\nsel=2 o=0\n");
}

/// A default item in the middle of the source order is still emitted as the
/// final fallback, so an explicit item after it remains reachable.
#[test]
fn sim_case_default_middle() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [1:0] sel;
    reg [1:0] o;
    always_comb begin
        case (sel)
            2'd0: o = 2'd0;
            default: o = 2'd3;
            2'd2: o = 2'd2;
        endcase
    end
    initial begin
        sel = 2'd2; #1 $display("sel=%0d o=%0d", sel, o);
        sel = 2'd1; #1 $display("sel=%0d o=%0d", sel, o);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim("llg_case_default_middle", sv).expect("simulation should run");
    assert_eq!(stdout, "sel=2 o=2\nsel=1 o=3\n");
}

/// Z literals on wires: `$display("%b")` prints 'z' for Z bits and 'x' for X
/// bits; `%h` folds per nibble (any X -> 'x', else any Z -> 'z').
#[test]
fn sim_z_literal_display() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic zz;
    logic [3:0] w;
    assign zz = 1'bz;
    assign w = 4'b10zx;
    initial begin
        #1 $display("zz=%b", zz);
        #1 $display("w=%b", w);
        #1 $display("wh=%h", w);
        $finish;
    end
endmodule
"#;
    // Hand-simulation:
    //   t=0  spawn order: comb(zz), comb(w), initial.  comb(zz) writes z into
    //        zz (its RHS is a constant; runs once); comb(w) writes 10zx.
    //   t=1  $display("zz=z") — the z written by the assign stays z (a
    //        single-driver continuous assignment writes Z into the net).
    //   t=2  $display("w=10zx") — %b prints 'z' for bit 1 and 'x' for bit 0.
    //   t=3  $display("wh=x") — the nibble 10zx contains X, so %h prints 'x'.
    let stdout = run_sim("llg_zdisp", sv).expect("z display simulation should run");
    assert_eq!(stdout, "zz=z\nw=10zx\nwh=x\n");
}

/// `===`/`!==` compare X and Z literally: X==X true, Z==Z true, X==Z false.
#[test]
fn sim_case_equality() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [3:0] a, b, c;
    reg r1, r2, r3;
    always_comb begin
        a = 4'b10xz;
        b = 4'b10xz;
        c = 4'b10xx;
        r1 = (a === b);
        r2 = (a === c);
        r3 = (a !== c);
    end
    initial begin
        #1 $display("r1=%b r2=%b r3=%b", r1, r2, r3);
        $finish;
    end
endmodule
"#;
    // Hand-simulation:
    //   t=0  comb evaluates: a === b (X==X at bit 1, Z==Z at bit 0) -> r1=1;
    //        a === c -> bit 0 is Z vs X -> r2=0; a !== c -> r3=1.
    //   t=1  $display("r1=1 r2=0 r3=1"); $finish.
    let stdout = run_sim("llg_caseq", sv).expect("case-equality simulation should run");
    assert_eq!(stdout, "r1=1 r2=0 r3=1\n");
}
