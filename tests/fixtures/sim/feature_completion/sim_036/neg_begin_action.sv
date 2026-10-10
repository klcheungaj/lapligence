// SIM-036 A03 negative: a pass or fail action of a deferred assertion is a
// single subroutine call; begin-end is a statement, not a call (IEEE
// 1800-2009 16.4).
module tb;
  initial begin
    assert #0 (1'b1) begin $display("a"); end
    #1 $finish(0);
  end
endmodule
