// IEEE 1800-2009 6.5: a[0:3] is continuously driven through an output port;
// the procedural write to a[1] overlaps it.
module src4(output logic [7:0] o [0:3]);
  assign o = '{8'd1, 8'd2, 8'd3, 8'd4};
endmodule
module tb;
  logic [7:0] a [0:65536];
  src4 u(.o(a[0:3]));
  initial a[1] = 8'hff;
  initial $finish(0);
endmodule
