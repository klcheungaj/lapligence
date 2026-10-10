// SIM-032 A03: a program generate item shall not contain what a program
// cannot (IEEE 1800-2009 Syntax 24-1 note 5, 24.3).
program p;
  logic v;
  if (1) begin : g
    always @* v = 1'b1;
  end
  initial #1 $display("v=%b", v);
endprogram

module tb;
  p p0();
endmodule
