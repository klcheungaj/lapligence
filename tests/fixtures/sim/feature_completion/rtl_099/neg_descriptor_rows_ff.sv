// IEEE 1800-2009 6.5 and 9.2.2.4: the slice b[2:3] spans both whole rows, so
// an always_ff write to b[3][7] overlaps the output-port driver.
module src_rows(output logic [7:0] o [0:1][0:15]);
  always_comb foreach (o[r, c]) o[r][c] = 8'(r + c);
endmodule
module tb;
  logic [7:0] b [0:4095][0:15];
  logic c;
  src_rows u(.o(b[2:3]));
  always_ff @(posedge c) b[3][7] <= 8'h1;
  initial $finish(0);
endmodule
