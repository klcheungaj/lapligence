// SIM-032 A03: a program shall not contain interface instances
// (IEEE 1800-2009 24.3).
interface ifc;
  logic a;
endinterface

program p;
  ifc i0();
  initial $display("p");
endprogram

module tb;
  p p0();
endmodule
