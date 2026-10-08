// SV 16.9.3: $fell of a shortreal has no least significant bit either.
module tb;
  logic clk = 1'b0;
  shortreal r = 0.5;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $fell(r, @(posedge clk)));
  initial #10 $finish;
endmodule
