// A19-D11: vpiDecompile spells each operand and operator once, separated by
// one space, with parentheses only where precedence needs them (37.55 detail
// 2). The companion prints the decompiled right-hand side of every
// continuous assignment.
module tb;
  typedef logic [3:0] nib_t;
  logic [7:0] a = 8'd9, b = 8'd4, c = 8'd2;
  logic signed [7:0] s = 8'sd5;
  int i = 3;
  real r = 2.5;
  logic [7:0] y0, y1, y2, y3, y4, y5, y6, y7, y8, y9, y10, y11, y12, y13;
  int z0, z1;
  real q0;
  assign y0 = -a;
  assign y1 = a - (b - c);
  assign y2 = (a - b) - c;
  assign y3 = (a + b) * c;
  assign y4 = a + b * c;
  assign y5 = a ? b : c ? a : b;
  assign y6 = {a[3:0], {2{b[1:0]}}};
  assign y7 = a[i +: 2] | b[7 -: 3] & ~c;
  assign y8 = 4'b10x1 ^ 8'hzz;
  assign y9 = 12'o17 + 255;
  assign y10 = signed'(a) >>> 1;
  assign y11 = 4'(a) + nib_t'(b);
  assign y12 = !(a == b) || &c;
  assign y13 = s ** 2;
  assign z0 = int'(r) - 1;
  assign z1 = 'x;
  assign q0 = r * 1.0e-9 + 0.5;
  initial begin
    #1 $d11;
    $finish;
  end
endmodule
