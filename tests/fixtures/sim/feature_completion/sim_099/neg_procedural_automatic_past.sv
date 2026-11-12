// 16.15.6.1: an automatic variable takes its value when the instance is
// queued; the history of a sampled value function is kept per assertion,
// outside the instance's attempt, so llg rejects such an argument.
module tb;
  logic clk = 0;
  logic [3:0] foo = 4'b1010;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    for (int i = 0; i < 4; i++) begin
      p1: assert property (@(posedge clk) $past(foo[i]));
    end
  end
  initial #12 $finish;
endmodule
