// ADV-012 checks: every timing check entry of Table 32-2 replaces the
// limits of the checks it maps to (IEEE 1800-2009 32.4.1, Table 32-2;
// IEEE 1364-2001 16.2.2). Each cell holds one check with limit 1, and each
// annotated limit turns a phase of stimulus into exactly one violation that
// limit 1 would not report (and one $skew event that limit 1 would).
// Oracles in readme.md.
`timescale 1ns/1ps

module SU (input clk, input d);
  reg n = 1'b0;
  specify
    $setup(d, posedge clk, 1, n);
  endspecify
endmodule

module HO (input clk, input d);
  reg n = 1'b0;
  specify
    $hold(posedge clk, d, 1, n);
  endspecify
endmodule

module SH (input clk, input d);
  reg n = 1'b0;
  specify
    $setuphold(posedge clk, d, 1, 1, n);
  endspecify
endmodule

module RC (input clk, input rst);
  reg n = 1'b0;
  specify
    $recovery(posedge rst, posedge clk, 1, n);
  endspecify
endmodule

module RM (input clk, input rst);
  reg n = 1'b0;
  specify
    $removal(posedge rst, posedge clk, 1, n);
  endspecify
endmodule

module RR (input clk, input rst);
  reg n = 1'b0;
  specify
    $recrem(posedge rst, posedge clk, 1, 1, n);
  endspecify
endmodule

module SK (input clk, input d);
  reg n = 1'b0;
  specify
    $skew(posedge clk, posedge d, 1, n);
  endspecify
endmodule

module WI (input clk);
  reg n = 1'b0;
  specify
    $width(posedge clk, 1, 0, n);
  endspecify
endmodule

module PE (input clk);
  reg n = 1'b0;
  specify
    $period(posedge clk, 1, n);
  endspecify
endmodule

module NC (input clk, input d);
  reg n = 1'b0;
  specify
    $nochange(posedge clk, d, 0, 0, n);
  endspecify
endmodule

module tb;
  reg [9:0] clk = 10'b0, d = 10'b0, rst = 10'b0;
  SU su (clk[0], d[0]);
  HO ho (clk[1], d[1]);
  SH sh (clk[2], d[2]);
  RC rc (clk[3], rst[3]);
  RM rm (clk[4], rst[4]);
  RR rr (clk[5], rst[5]);
  SK sk (clk[6], d[6]);
  WI wi (clk[7]);
  PE pe (clk[8]);
  NC nc (clk[9], d[9]);
  initial begin
    $sdf_annotate("checks.sdf", tb);
    $monitor("%0t n=%b", $realtime,
             {nc.n, pe.n, wi.n, sk.n, rr.n, rm.n, rc.n, sh.n, ho.n, su.n});
    // su: data 3 before the clock (setup 6).
    #100 d[0] = 1'b1;
    #3 clk[0] = 1'b1;
    #17 clk[0] = 1'b0;
    // ho: data 4 after the clock (hold 6).
    #80 clk[1] = 1'b1;
    #4 d[1] = 1'b1;
    #16 clk[1] = 1'b0;
    // sh: data 3 before and 4 after the clock (setup 6, hold 6).
    #80 d[2] = 1'b1;
    #3 clk[2] = 1'b1;
    #4 d[2] = 1'b0;
    #13 clk[2] = 1'b0;
    // rc: clock 5 after the reset (recovery 7).
    #80 rst[3] = 1'b1;
    #5 clk[3] = 1'b1;
    #15 rst[3] = 1'b0;
    // rm: reset 5 after the clock (removal 8).
    #80 clk[4] = 1'b1;
    #5 rst[4] = 1'b1;
    #15 rst[4] = 1'b0;
    // rr: clock 5 after the reset (recovery 7), then reset 5 after the
    // clock (removal 8).
    #80 rst[5] = 1'b1;
    #5 clk[5] = 1'b1;
    #5 clk[5] = 1'b0;
    #10 rst[5] = 1'b0;
    #20 clk[5] = 1'b1;
    #5 rst[5] = 1'b1;
    #15 rst[5] = 1'b0;
    // sk: data 2, then 4 after the clock (skew 3).
    #40 clk[6] = 1'b1;
    #2 d[6] = 1'b1;
    #8 clk[6] = 1'b0;
    #2 d[6] = 1'b0;
    #8 clk[6] = 1'b1;
    #4 d[6] = 1'b1;
    #26 clk[6] = 1'b0;
    // wi: a high phase of 7, then of 15 (width 12).
    #50 clk[7] = 1'b1;
    #7 clk[7] = 1'b0;
    #13 clk[7] = 1'b1;
    #15 clk[7] = 1'b0;
    // pe: rising edges 30, then 45 apart (period 40).
    #65 clk[8] = 1'b1;
    #10 clk[8] = 1'b0;
    #20 clk[8] = 1'b1;
    #10 clk[8] = 1'b0;
    #35 clk[8] = 1'b1;
    #10 clk[8] = 1'b0;
    // nc: data 2 after the trailing edge (offsets 2 and 3).
    #15 clk[9] = 1'b1;
    #10 clk[9] = 1'b0;
    #2 d[9] = 1'b1;
    #88 $finish;
  end
endmodule
