// SV 16.6.1: string operands are excluded from sampled expressions.
module tb;
  logic clk = 1'b0;
  string s = "a";
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $stable(s));
  initial #10 $finish;
endmodule
