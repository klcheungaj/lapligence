// RTL-012: omitted net-array formals are pulled per cell beside the formal's
// internal drivers; connected net-array inputs resolve the same way (IEEE
// 1364-2001 19.9, 7.10; IEEE 1800-2009 22.9, 23.3.3). Hand-derived output.
`unconnected_drive pull1
module child(input wire [1:0] a[2], input wire b[3], input wire [1:0] c[0:1][2:0]);
  assign a[1] = 2'b0z;
  assign (strong0, strong1) b[2] = 1'b0;
  assign c[1][0] = 2'b0z;
endmodule
`nounconnected_drive
module tb;
  wire [1:0] p[1:0][0:2];
  assign p[1][0] = 2'b00;
  assign p[1][1] = 2'b01;
  assign p[1][2] = 2'b10;
  assign p[0][0] = 2'b11;
  assign p[0][1] = 2'bz1;
  assign p[0][2] = 2'b1x;
  child u(.c(p));
  initial begin
    #1;
    $display("%b %b %v %v | %v %v %v", u.a[0], u.a[1], u.a[1][1], u.a[1][0], u.b[0], u.b[1], u.b[2]);
    $display("%b %b %b %b %b %b", u.c[0][2], u.c[0][1], u.c[0][0], u.c[1][2], u.c[1][1], u.c[1][0]);
    $finish(0);
  end
endmodule
