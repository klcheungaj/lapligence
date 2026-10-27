// ADV-015 A01 boundary: two checker procedures assign disjoint bits of one
// variable (17.7.1 allows it), a named block groups a nonblocking
// assignment with a deferred and a concurrent assertion (17.5), the checker
// is instantiated twice with independent state, and an unused checker
// declaration costs nothing (IEEE 1800-2009 17.2, 17.5, 17.7.1).
`timescale 1ns / 1ns
checker parts(logic [2:0] v, event clk);
  bit [2:0] b = 3'd0;
  always @(clk) b[0] <= v[0];
  always @(clk) begin : upper
    b[2:1] <= v[2:1];
    d_low: assert #0 (v != 3'd7) else $display("%0t %m v=7", $time);
    a_follow: assert property (@clk ##1 b == $past(v))
      else $display("%0t %m b=%0d", $time, b);
  end
  final $display("%m b=%0d", b);
endchecker

checker unused_check(logic a);
  a1: assert property (@(posedge a) a);
endchecker

module tb;
  logic clk = 1'b0;
  logic [2:0] v = 3'd0, w = 3'd5;
  always #5 clk = ~clk;
  parts p1(v, posedge clk);
  parts p2(w, negedge clk);
  initial begin
    #12 v = 3'd3;
    #10 v = 3'd7;
    #10 v = 3'd6;
    #12 $finish;
  end
endmodule
