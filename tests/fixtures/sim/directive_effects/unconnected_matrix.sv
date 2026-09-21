// IEEE 1364-2001 section 19.9 and IEEE 1800-2009 section 22.9: pull0,
// pull1 and nounconnected_drive affect omitted packed input values after
// elaboration.
`unconnected_drive pull0
module child0(input wire [3:0] i, output wire [3:0] o);
  assign o = i;
endmodule
`unconnected_drive pull1
module child1(input wire [3:0] i, output wire [3:0] o);
  assign o = i;
endmodule
`nounconnected_drive
module childz(input wire [3:0] i, output wire [3:0] o);
  assign o = i;
endmodule
module tb;
  wire [3:0] p0, p1, pz;
  child0 u0(.o(p0));
  child1 u1(.o(p1));
  childz uz(.o(pz));
  initial begin
    #0;
    $display("p0=%h p1=%h pz=%h", p0, p1, pz);
    $finish;
  end
endmodule
