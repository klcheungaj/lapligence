// IEEE 1800-2009 6.5 and 23.3.3.2: an output port bound to a 65,537-cell row is
// a continuous driver of that row; an always_ff write inside it overlaps.
module src_row(output logic [7:0] o [65537]);
  initial o[0] = 8'h1;
endmodule
module tb;
  logic [7:0] two [2][65537];
  logic [7:0] d; logic c;
  src_row u(.o(two[0]));
  always_ff @(posedge c) two[0][3] <= d;
  initial $finish(0);
endmodule
