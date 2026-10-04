// SIM-002: $printtimescale takes a module instance (SV2009 20.4.1); a
// generate block is not one.
module tb;
  if (1) begin : g
  end
  initial $printtimescale(g);
endmodule
