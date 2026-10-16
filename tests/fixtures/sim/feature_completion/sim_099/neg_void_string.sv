// SIM-099 limit: the discarded string result of a void-cast $sformatf is
// rejected at its source position, never routed to VPI dispatch.
module tb;
  initial begin
    void'($sformatf("%0d", 3));
  end
endmodule
