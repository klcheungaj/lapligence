// $global_clock as a clocking event (IEEE 1800-2009 14.14) is not
// implemented: the run stops at the call instead of reporting it as a user
// system function.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  global clocking @(posedge clk); endclocking
  assert property (@($global_clock) a |-> b);
  initial #1 $finish;
endmodule
