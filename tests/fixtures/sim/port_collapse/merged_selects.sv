// Input and output net ports whose actual is a constant select, a net-array
// element or a concatenation of nets, merged bit by bit with their formal
// (decision A21-D5): drivers on both sides, strengths, dominating types,
// disjoint and overlapping ports on one vector, chains, forces.
`timescale 1ns/1ns
module coerced(input wire n);
  assign n = 1'b0;
endmodule

module coerced2(input wire [1:0] n);
  assign n[0] = 1'b0;
endmodule

module plain(input wire n);
endmodule

module pulled(input tri0 n);
endmodule

module drive1(output wire o);
  assign o = 1'b1;
endmodule

module weak_drive(output wire o);
  assign (weak0, weak1) o = 1'b1;
endmodule

module drive10(output wire [1:0] o);
  assign o = 2'b10;
endmodule

module mid(input wire m);
  coerced g(.n(m));
endmodule

module watch(input wire m);
  wire k;
  assign k = m;
endmodule

module tb;
  logic x;
  wire [3:0] v, o, f;
  tri1 [1:0] t;
  wire [3:0] d, a, b;
  wire w [0:3];
  wire [3:0] wa [0:1];
  wire c, e, g, h;
  assign v = {4{x}};
  assign o = {4{x}};
  assign f = {4{x}};
  assign w[0] = x;
  assign w[1] = x;
  assign wa[0] = {4{x}};
  assign c = x;
  assign e = x;
  assign b[1] = 1'b0;
  coerced u0(.n(v[1]));
  coerced u1(.n(o[0]));
  coerced u2(.n(o[3]));
  coerced2 u3(.n(o[2:1]));
  plain u4(.n(t[1]));
  pulled u5(.n(d[2]));
  coerced u6(.n(w[1]));
  coerced2 u7(.n(wa[0][1:0]));
  coerced2 u8(.n({c, e}));
  drive1 u9(.o(a[1]));
  weak_drive u10(.o(a[2]));
  drive1 u11(.o(b[1]));
  drive10 u12(.o({g, h}));
  drive1 u13(.o(w[2]));
  mid u14(.m(v[2]));
  watch u15(.m(f[1]));
  initial begin
    x = 1'b1;
    #1 $display("bit select: v=%b u0.n=%b %v", v, u0.n, u0.n);
    $display("disjoint and overlapping: o=%b u1.n=%b u2.n=%b u3.n=%b", o, u1.n, u2.n, u3.n);
    $display("pulled bit: t=%b u4.n=%b %v", t, u4.n, u4.n);
    $display("tri0 formal: d=%b %v u5.n=%v", d, d[2], u5.n);
    $display("array element: w0=%b w1=%b u6.n=%b", w[0], w[1], u6.n);
    $display("array element part: wa0=%b u7.n=%b", wa[0], u7.n);
    $display("concatenation: c=%b e=%b u8.n=%b", c, e, u8.n);
    $display("output bits: a=%b %v %v", a, a[1], a[2]);
    $display("second driver: b=%b u11.o=%b", b, u11.o);
    $display("output concatenation: g=%b h=%b u12.o=%b", g, h, u12.o);
    $display("output element: w2=%v u13.o=%b", w[2], u13.o);
    $display("chain: v=%b u14.m=%b u14.g.n=%b", v, u14.m, u14.g.n);
    force u15.m = 1'b0;
    #1 $display("forced formal: f=%b u15.m=%b u15.k=%b", f, u15.m, u15.k);
    release u15.m;
    #1 $display("released: f=%b u15.m=%b", f, u15.m);
    x = 1'b0;
    #1 $display("x=0: v=%b o=%b u0.n=%b u3.n=%b w1=%b u8.n=%b", v, o, u0.n, u3.n, w[1], u8.n);
    $finish(0);
  end
endmodule
