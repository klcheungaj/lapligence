// Net ports of a model reached through VPI merge as without VPI (decision
// A21-D6); a port from a variable keeps its link. vpi_merged_ports.c checks
// both names of each port, and the port delays a merged port refuses; a
// model generated with --vpi-port-delays keeps every port separate.
module child(input w, input c, output o);
  reg ro = 1'b0;
  assign o = ro;
endmodule

module tb;
  reg drv = 1'b0;
  reg clk = 1'b0;
  wire wn, on;
  assign wn = drv;
  child u(.w(wn), .c(clk), .o(on));
  initial begin
    #1 $ports_check;
    #1 drv = 1'b1;
    #1 $ports_check;
    $finish;
  end
endmodule
