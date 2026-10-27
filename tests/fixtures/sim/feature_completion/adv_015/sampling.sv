// ADV-015 A01: checker procedures and action blocks run in the Reactive
// region and read formals as the sampled values of their actuals; checker
// variable updates mature in the Re-NBA region, so an action block in the
// same time step still sees the old value (IEEE 1800-2009 17.3, 17.7.3).
`timescale 1ns / 1ns
checker counter(logic a, event clk);
  bit [3:0] cnt = 4'd0;
  always @(clk) cnt <= cnt + 4'd1;
  a1: assert property (@clk a) else $display("%0t %m fail cnt=%0d", $time, cnt);
  final $display("%m cnt=%0d", cnt);
endchecker

checker follower(logic [3:0] d, event clk, logic [3:0] e = 4'd9);
  logic [3:0] seen = 4'd0, seen_e = 4'd0;
  always @(clk) begin
    seen <= d;
    seen_e <= e;
  end
  a2: assert property (@clk d == seen)
    else $display("%0t %m d=%0d seen=%0d", $time, d, seen);
  final $display("%m seen=%0d seen_e=%0d", seen, seen_e);
endchecker

module tb;
  logic clk = 1'b0, x = 1'b1;
  logic [3:0] q = 4'd0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 4'd1;
  counter c(x, posedge clk);
  follower f(q, posedge clk);
  initial begin
    #22 x = 1'b0;
    #20 x = 1'b1;
    #30 $finish;
  end
endmodule
