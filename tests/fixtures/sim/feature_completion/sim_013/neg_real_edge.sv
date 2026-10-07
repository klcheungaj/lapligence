// SIM-013 A03 negative: posedge/negedge/edge are defined only for integral
// values; a real operand of an edge event is illegal (SV 6.12.1, 9.4.2).
module tb;
  real r = 0.0;
  initial begin
    @(negedge r);
    $display("unreachable");
  end
  initial #1 r = -1.0;
endmodule
