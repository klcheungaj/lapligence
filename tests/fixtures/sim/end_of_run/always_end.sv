// An always procedure whose event never occurs, and one parked behind a
// clock that never toggles, are permanent waiters, not a deadlock.
module tb;
  logic a = 1'b1, b, clk = 1'b0, c;
  always @(a) b = a;
  always @(posedge clk) c <= a;
  initial #1 $display("initial done");
  final $display("final b=%b c=%b", b, c);
endmodule
