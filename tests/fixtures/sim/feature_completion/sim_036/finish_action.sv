// SIM-036 A03: $finish as a deferred assertion action executes once, in the
// Reactive region of the issuing step, after the issuing process has run
// on; the final procedure then runs (IEEE 1800-2009 16.4, 20.2, 9.2.3).
module tb;
  initial begin
    $display("before");
    assert #0 (1'b1) $finish;
    $display("same step");
    #1 $display("BAD after finish");
  end
  final $display("final");
endmodule
