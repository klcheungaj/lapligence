// SIM-032 A03: a program shall not contain always procedures
// (IEEE 1800-2009 24.3).
program p;
  logic v;
  always @* v = 1'b1;
  initial #1 $display("v=%b", v);
endprogram

module tb;
  p p0();
endmodule
