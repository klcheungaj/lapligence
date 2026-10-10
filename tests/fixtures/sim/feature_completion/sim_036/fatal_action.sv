// SIM-036 A03: $fatal as a deferred assertion action ends the run in the
// Reactive region of the issuing step (IEEE 1800-2009 16.4, 20.10).
module tb;
  int v;
  initial begin
    v = 1;
    assert #0 (1'b0) else $fatal(0, "fatal v=%0d", v);
    v = 2;
    $display("issued v=%0d", v);
    #1 $display("BAD after fatal");
  end
  final $display("final");
endmodule
