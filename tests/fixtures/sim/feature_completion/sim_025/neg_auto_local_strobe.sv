// SIM-025 A03: a loop variable is automatic (SV 12.7.1, 6.21), so a deferred
// report cannot name it (SV 13.3.2).
module tb;
  initial begin
    for (int i = 0; i < 2; i++) $strobe("i=%0d", i);
    #1 $finish(0);
  end
endmodule
