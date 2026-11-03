// Decision A21-D5: an input or output net port whose actual is a constant
// bit or part select of a net, a net-array element or a concatenation of
// nets is merged with its formal bit by bit (MSB to MSB) into simulated
// nets, as for a whole net (A21-D4), whether or not either side has other
// drivers. A child driving its input formal reaches the selected bit of the
// parent's net; two ports on disjoint or overlapping bits of one vector
// each merge their own bits; an undriven pulled bit shows its pull strength
// through the formal and a weak output driver stays weak on the selected
// bit; a tri0 formal dominates a selected wire bit; a whole port below a
// merged formal joins the same net; a force of the formal forces the
// selected bit.
//
// IEEE 1800-2009 23.3.3 (SystemVerilog-1800-2009.txt L41452-41454):
//   "Each port connection shall be a continuous assignment of source to sink,
//   where one connected item shall be a signal source and the other shall be
//   a signal sink. The assignment shall be a continuous assignment from
//   source to sink for input or output ports."
// IEEE 1800-2009 23.3.3.1 (L41461-41462):
//   "A port that is declared as input (output) but used as an output (input)
//   or inout may be coerced to inout. If not coerced to inout, a warning
//   shall be issued."
// IEEE 1800-2009 23.3.3.7 (L41592-41598):
//   "When different net types are connected through a module port, the nets on
//   both sides of the port can take on the same type. [...] It is permissible
//   to merge the dominating and dominated nets into a single net, whose type
//   shall be that of the dominating net."
// IEEE 1800-2009 10.6.2 (L13397-13399):
//   "A force procedural statement on a net shall override all drivers of the
//   net-gate outputs, module outputs, and continuous assignments-until a
//   release procedural statement is executed on the net."
//
// The text does not say whether a selected actual is merged or copied. llg
// follows the industry-standard behaviour and merges the selected bits.
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

module weak_drive(output wire o);
  assign (weak0, weak1) o = 1'b1;
endmodule

module drive10(output wire [1:0] o);
  assign o = 2'b10;
endmodule

module mid(input wire m);
  coerced g(.n(m));
endmodule

module tb;
  logic x;
  wire [3:0] v, o, f, a;
  tri1 [1:0] t;
  wire [3:0] d;
  wire w [0:1];
  wire c, e, g, h;
  assign v = {4{x}};
  assign o = {4{x}};
  assign f = {4{x}};
  assign w[0] = x;
  assign w[1] = x;
  assign c = x;
  assign e = x;
  assign g = x;
  coerced u0(.n(v[1]));
  coerced u1(.n(o[0]));
  coerced2 u2(.n(o[2:1]));
  plain u3(.n(t[1]));
  pulled u4(.n(d[2]));
  coerced u5(.n(w[1]));
  coerced2 u6(.n({c, e}));
  weak_drive u7(.o(a[2]));
  drive10 u8(.o({g, h}));
  mid u9(.m(v[3]));
  plain u10(.n(f[1]));
  initial begin
    x = 1'b1;
    #1 $display("bit select: v=%b u0.n=%b %v", v, u0.n, u0.n);
    $display("two ports: o=%b u1.n=%b u2.n=%b", o, u1.n, u2.n);
    $display("pulled bit: u3.n=%v", u3.n);
    $display("tri0 formal: d=%b u4.n=%v", d, u4.n);
    $display("array element: w0=%b w1=%b u5.n=%b", w[0], w[1], u5.n);
    $display("concatenation: c=%b e=%b u6.n=%b", c, e, u6.n);
    $display("weak output: a=%b %v", a, a[2]);
    $display("output concatenation: g=%b h=%b u8.o=%b", g, h, u8.o);
    $display("chain: u9.m=%b u9.g.n=%b", u9.m, u9.g.n);
    force u10.n = 1'b0;
    #1 $display("forced formal: f=%b u10.n=%b", f, u10.n);
    release u10.n;
    #1 $display("released: f=%b u10.n=%b", f, u10.n);
    x = 1'b0;
    #1 $display("x=0: v=%b o=%b w1=%b u6.n=%b g=%b", v, o, w[1], u6.n, g);
    $finish;
  end
endmodule
