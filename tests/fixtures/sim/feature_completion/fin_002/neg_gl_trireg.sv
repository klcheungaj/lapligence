// FIN-002 gate-level negative: charge storage is not simulated; a netlist
// that keeps a value on a trireg node stops before C generation.
`timescale 1ns / 1ns
module tb;
  reg d, g;
  wire dn;
  trireg (medium) store;
  assign dn = d;
  tranif1 (dn, store, g);
  initial begin
    d = 1'b1;
    g = 1'b1;
    #1 g = 1'b0;
    #1 $display("store=%v", store);
    $finish;
  end
endmodule
