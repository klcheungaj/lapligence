// SV2009 7.4.6, 11.5, 23.2.2.3: every step clips to its immediate base.
module leaf(ref logic [3:0] d, ref logic [0:3] a);
 initial begin
  #1;
  d[1 +: 2] = 2'b10;
  a[2 -: 2] = 2'b11;
  d[-1] = 1;
  a[4] = 1;
 end
endmodule
module middle(ref logic [7:0] d, ref logic [0:7] a);
 leaf l(d[6 -: 4], a[6 -: 4]);
 logic [31:0] unknown_x, unknown_z;
 initial begin
  #2;
  $display("before %h %h", d, a);
  d[-1 +: 3] = 3'b111;
  a[6 +: 4] = 4'b1010;
  $display("partial %b %b", d[-1 +: 3], a[6 +: 4]);
  d[32'hffffffff -: 2] = 0;
  unknown_x = 'x; unknown_z = 'z;
  d[unknown_x +: 2] = 0;
  a[unknown_z -: 2] = 0;
  $display("unknown %b %b", d[unknown_x], a[unknown_z]);
 end
endmodule
module tb;
 logic [15:0] d = 0;
 logic [0:15] a = 0;
 middle m(d[15:8], a[0:7]);
 initial begin #3; $display("root %h %h", d, a); $finish(0); end
endmodule
