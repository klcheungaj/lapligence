// SIM-032 A03: a program shall not contain module instances
// (IEEE 1800-2009 24.3).
module leaf;
endmodule

program p;
  leaf l0();
  initial $display("p");
endprogram

module tb;
  p p0();
endmodule
