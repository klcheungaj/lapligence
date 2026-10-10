// SIM-036 A03: a deferred assertion in a final procedure (IEEE 1800-2009
// 9.2.3, 16.4). The final procedure is never suspended and no Observed
// region follows it; llg executes its reports when that final procedure
// returns (decision S36-D4).
module tb;
  int v = 3;
  final begin
    assert #0 (v == 4) else $display("final fail v=%0d", v);
    v = 5;
    $display("final end v=%0d", v);
  end
  initial #1 $finish(0);
endmodule
