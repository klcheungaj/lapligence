// 16.15.6.1: a const cast in a procedural concurrent assertion takes its
// value when the instance is queued; llg keeps no queued values.
module tb;
  logic clk = 0;
  logic [3:0] foo = 4'b1010;
  int j = 1;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    c1: assert property (@(posedge clk) foo[const'(j)]);
  end
  initial #12 $finish;
endmodule
