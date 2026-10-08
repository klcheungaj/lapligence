// SV 16.9.3: an X tick count names no clock tick.
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $past(v, 1'bx));
  initial #10 $finish;
endmodule
