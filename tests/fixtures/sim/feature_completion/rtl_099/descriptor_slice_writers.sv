// IEEE 1800-2009 6.5 and 23.3.3.2: an output port bound to a constant slice
// is a continuous driver of exactly the slice's cells. Disjoint procedural
// writers of the same descriptor-backed arrays stay legal.
module src4(output logic [7:0] o [0:3], input logic [7:0] base);
  assign o[0] = base;
  assign o[1] = base + 8'd1;
  assign o[2] = base + 8'd2;
  assign o[3] = base + 8'd3;
endmodule
module src_rows(output logic [7:0] o [0:1][0:15], input logic [7:0] base);
  always_comb
    foreach (o[r, c]) o[r][c] = base + 8'(r * 16 + c);
endmodule
module tb;
  logic [7:0] a [0:65536];
  logic [7:0] b [0:4095][0:15];
  logic [7:0] base;
  src4 lo(.o(a[0:3]), .base(base));
  src4 hi(.o(a[65533:65536]), .base(8'h80));
  src_rows rows(.o(b[2:3]), .base(8'h40));
  initial begin
    base = 8'h10;
    a[4] = 8'haa;
    a[65532] = 8'hbb;
    b[1][15] = 8'hcc;
    b[4][0] = 8'hdd;
    #1 $display("%h %h %h %h %h", a[0], a[3], a[4], a[65532], a[65533]);
    $display("%h %h %h %h %h", a[65536], b[1][15], b[2][0], b[3][15], b[4][0]);
    base = 8'h20;
    #1 $display("%h %h %h", a[0], a[3], a[5]);
    $finish(0);
  end
endmodule
