// IEEE 1800-2009 16.9.3: $rose/$fell test the least significant bit, which a
// real expression does not have.
module tb;
  real r;
  logic clk = 0;
  always #5 clk = ~clk;
  always @(posedge clk) if ($rose(r)) $display("rose");
  initial begin
    r = 1.0;
    #20 $finish(0);
  end
endmodule
