// SV 16.9.3: number_of_ticks shall be 1 or greater.
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $past(v, 0));
  initial #10 $finish;
endmodule
