// IEEE 1364-2001 section 19.5 and IEEE 1800-2009 section 22.4: an
// unavailable include is rejected before elaboration.
`include "missing_syn017.svh"
module tb;
  initial begin
    $finish;
  end
endmodule
