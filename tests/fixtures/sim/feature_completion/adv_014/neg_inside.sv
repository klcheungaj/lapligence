// FIN-001 A3 limit: in an assertion with global clocking future sampled value
// functions (IEEE 1800-2009 16.9.4), only operators, selects, casts and
// concatenations may surround a future call; `inside` is rejected.
module tb;
  logic clk = 1'b0;
  logic [3:0] a = 4'h0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) $future_gclk(a) inside {4'h0, 4'h1});
  always #5 clk = ~clk;
  initial #20 $finish;
endmodule
