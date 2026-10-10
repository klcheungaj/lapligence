// SV 16.6.1 (SystemVerilog-1800-2009.txt L21576): shortreal is a noninteger
// type, so $fell of a shortreal is rejected.
module tb;
  logic clk = 1'b0;
  shortreal r = 0.5;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $fell(r, @(posedge clk)));
  initial #10 $finish;
endmodule
