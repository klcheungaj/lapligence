// SIM-032 A03: calling program subroutines from design modules is illegal
// (IEEE 1800-2009 24.5).
program p;
  task t;
    $display("t");
  endtask
  initial #1;
endprogram

module tb;
  p p0();
  initial p0.t();
endmodule
