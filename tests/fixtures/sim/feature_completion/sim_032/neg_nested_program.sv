// SIM-032 A03: a program shall not contain declarations or instances of
// other programs (IEEE 1800-2009 24.3).
program p;
  program q;
    initial $display("q");
  endprogram
  initial $display("p");
endprogram

module tb;
  p p0();
endmodule
