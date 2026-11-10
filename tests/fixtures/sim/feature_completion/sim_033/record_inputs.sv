`timescale 1ns/1ns
// A record clockvar with a member array of N cells: 16 cells make the
// member one column, 3 cells one leaf per cell. Both forms sample the same
// whole value (SV 14.13).
`ifndef N
`define N 3
`endif
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  typedef struct { int a; int m [`N]; logic [3:0] t; } r_t;
  r_t r;
  r_t rs [2];
  int i = 1;
  clocking cb @(posedge clk);
    input r, rs;
    input #2 rk = r;
  endclocking
  initial begin
    r.a = 1;
    r.t = 4'h2;
    foreach (r.m[k]) r.m[k] = k == `N - 1 ? 12 : k + 10;
    rs[1].m[`N-1] = 5;
    #7 r.m[i] = 77; r.a = 3; rs[1].m[`N-1] = 55; rs[0].t = 4'h9;
    #6 r.m[`N-1] = 88;
  end
  always @(cb)
    $display("%0t r %0d %0d %0d %h rs %0d %h rk %0d %0d", $time, cb.r.a, cb.r.m[i],
             cb.r.m[`N-1], cb.r.t, cb.rs[1].m[`N-1], cb.rs[0].t, cb.rk.m[i],
             cb.rk.m[`N-1]);
  initial #26 $finish;
endmodule
