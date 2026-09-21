// IEEE 1364-2001 section 19.2 and IEEE 1800-2009 section 22.8:
// default_nettype none
// rejects an implicit net at the elaboration boundary.
`default_nettype none
module tb;
  wire known;
  assign implicit_wire = known;
  initial begin
    $finish;
  end
endmodule
