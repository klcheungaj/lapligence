// SIM-032 A03: references to program signals from outside any program block
// shall be an error (IEEE 1800-2009 24.3).
program p;
  int v = 3;
  initial #1;
endprogram

module tb;
  p p0();
  initial $display("v=%0d", p0.v);
endmodule
