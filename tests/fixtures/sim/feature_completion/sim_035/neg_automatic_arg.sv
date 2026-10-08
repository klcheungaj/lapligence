// SV 16.9.3: variables in sampled-value arguments shall be static.
module tb;
  logic clk = 1'b0;
  always #5 clk = ~clk;
  initial begin
    automatic int k = 3;
    @(posedge clk) $display("%0d", $past(k, 1, , @(posedge clk)));
    $finish;
  end
endmodule
