// ADV-015b A02: an unsuccessful solution attempt (IEEE 1800-2009 17.7.2).
// The assume set of `stuck` has no solution, so "any values may be chosen"
// and the unsatisfied assumption fails when it is checked in the Observed
// region, at every clock event; nothing claims that it holds. The set of
// `free` is independent and keeps being solved. An X state value in an
// assumption makes no attempt fail with an error: the assumption is checked.
`timescale 1ns / 1ns
checker stuck(event clk);
  rand bit [3:0] q;
  u1: assume property (@clk q == 4'd5 && q == 4'd6) else $display("%0t u1 failed", $time);
endchecker
checker free(logic [3:0] s, event clk);
  rand bit [3:0] p;
  u2: assume property (@clk p == s + 4'd1) else $display("%0t u2 failed", $time);
  c2: cover property (@clk s !== 4'bx) $display("%0t p=%0d", $time, p);
endchecker
module tb;
  logic clk = 1'b0;
  logic [3:0] s;
  stuck a(posedge clk);
  free b(s, posedge clk);
  always #5 clk = ~clk;
  initial begin
    #12 s = 4'd3;
    #10 s = 4'd8;
    #10 $finish;
  end
endmodule
