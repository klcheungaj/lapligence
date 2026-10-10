// SIM-032 A03: a program shall not contain primitives
// (IEEE 1800-2009 24.3).
program p;
  wire a, b;
  and g0(a, b, b);
  initial $display("p");
endprogram

module tb;
  p p0();
endmodule
