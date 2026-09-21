// IEEE 1364-2001 sections 19.2 and 19.6; IEEE 1800-2009 sections 22.3
// and 22.8:
// resetall restores implicit-net admission after default_nettype none.
`default_nettype none
`resetall
module tb;
  wire known;
  assign implicit_wire = known;
  initial begin
    #0;
    $display("implicit=%b", implicit_wire);
    $finish;
  end
endmodule
