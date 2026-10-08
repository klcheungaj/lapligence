// SV 16.9.3: a negative number_of_ticks is invalid.
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $past(v, -1));
  initial #10 $finish;
endmodule
