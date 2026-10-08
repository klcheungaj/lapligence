// SV 16.6.1: class handles are excluded from sampled expressions.
module tb;
  class C;
  endclass
  logic clk = 1'b0;
  C c = new;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $changed(c));
  initial #10 $finish;
endmodule
