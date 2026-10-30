// always_comb and always_ff processes are permanent waiters as well.
module tb;
  logic a = 1'b1, clk = 1'b0, y, q;
  always_comb y = ~a;
  always_ff @(posedge clk) q <= a;
  initial #1 $display("initial done");
  final $display("final y=%b q=%b", y, q);
endmodule
