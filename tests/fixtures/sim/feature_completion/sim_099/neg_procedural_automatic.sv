// 16.15.6.1: an automatic variable in a procedural concurrent assertion
// takes its value when the instance is queued; llg keeps no queued values.
module tb;
  logic clk = 0;
  logic [3:0] foo = 4'b1010;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    for (int i = 0; i < 4; i++) begin
      c1: assert property (@(posedge clk) foo[i]);
    end
  end
  initial #12 $finish;
endmodule
