// IEEE 1800-2009 6.5: two output ports drive overlapping slices a[0:3] and
// a[3:6] of one variable.
module src4(output logic [7:0] o [0:3]);
  assign o = '{8'd1, 8'd2, 8'd3, 8'd4};
endmodule
module tb;
  logic [7:0] a [0:65536];
  src4 u0(.o(a[0:3]));
  src4 u1(.o(a[3:6]));
  initial $finish(0);
endmodule
