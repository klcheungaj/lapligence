// A legal fixed unpacked array argument (SV 16.6.1) whose sampled history is
// not implemented; it is rejected explicitly, not flattened.
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[2] = '{4'h1, 4'h2};
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $changed(arr));
  initial #10 $finish;
endmodule
