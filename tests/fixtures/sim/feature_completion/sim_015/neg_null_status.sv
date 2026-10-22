// SIM-015: status() through a null process handle inside an expression is a
// null object access (SV 8.4, 9.7): the run ends at the call, so neither the
// rest of the display that made it nor the next statement runs; the final
// procedure still runs (decision S99-D11).
module tb;
  process p;
  initial begin
    p = null;
    $display("before");
    $display("status %0d", p.status());
    $display("FAIL continued after null status");
  end
  final $display("final");
endmodule
