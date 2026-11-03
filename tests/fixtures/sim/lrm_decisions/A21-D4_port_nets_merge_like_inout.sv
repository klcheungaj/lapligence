// Decision A21-D4: an input or output port between two nets whose sink has
// drivers of its own or a type of its own is merged with its source into
// one simulated net, as an inout connection is. A net the child drives
// through its input port (port coercion), an output actual with a second
// driver and a net between two child outputs hold the drivers of both
// sides, and both names show the resolved value. A tri0 formal, a supply0
// formal, a tri1 actual or a wired formal give the merged net the type
// Table 23-1 selects (an internal tri0, tri1 or supply0 dominates an
// external wire; a wor actual dominates a wand formal). A force through
// either name forces the merged net.
//
// IEEE 1800-2009 23.3.3.1 (SystemVerilog-1800-2009.txt L41461-41462):
//   "A port that is declared as input (output) but used as an output (input)
//   or inout may be coerced to inout. If not coerced to inout, a warning
//   shall be issued."
// IEEE 1800-2009 23.3.3.7 (L41592-41598):
//   "When different net types are connected through a module port, the nets on
//   both sides of the port can take on the same type. The resulting net type
//   can be determined as shown in Table 23-1. [...] It is permissible to merge
//   the dominating and dominated nets into a single net, whose type shall be
//   that of the dominating net. The resulting net is called the simulated
//   net, and the dominated net is called a collapsed net."
// IEEE 1800-2009 Table 23-1 (L41613-41649): internal tri0, tri1 and supply0
//   rows read "internal" under an external wire; the wand row reads
//   "external, warn" under an external wor; the wire row reads "external".
// IEEE 1800-2009 10.6.2 (L13397-13399):
//   "A force procedural statement on a net shall override all drivers of the
//   net-gate outputs, module outputs, and continuous assignments-until a
//   release procedural statement is executed on the net."
//
// The text allows a continuous assignment from source to sink (each side
// then resolves its own drivers) or a merged net. llg follows the
// industry-standard behaviour and merges.
module coerced(input wire n);
  assign n = 1'b0;
endmodule

module drv(output wire o, input logic d);
  assign o = d;
endmodule

module mid(input wire m);
  coerced g(.n(m));
endmodule

module pulled(input tri0 t);
endmodule

module quiet(output wire o);
endmodule

module tied(input supply0 s);
endmodule

module anded(input wand a);
  assign a = 1'b0;
endmodule

module tb;
  logic x;
  wire c, m, m2, cm, z, s;
  tri1 p;
  wor wo;
  assign c = x;
  assign m = 1'b0;
  assign cm = x;
  assign s = x;
  assign wo = x;
  assign wo = 1'b0;
  coerced u0(.n(c));
  drv u1(.o(m), .d(x));
  drv u2(.o(m2), .d(x));
  drv u3(.o(m2), .d(1'b0));
  mid u4(.m(cm));
  pulled u5(.t(z));
  quiet u6(.o(p));
  tied u7(.s(s));
  anded u8(.a(wo));
  initial begin
    x = 1'b1;
    #1 $display("coerced input: c=%b u0.n=%b %v", c, u0.n, u0.n);
    $display("second driver: m=%b u1.o=%b", m, u1.o);
    $display("two outputs: m2=%b u2.o=%b u3.o=%b", m2, u2.o, u3.o);
    $display("chain: cm=%b u4.m=%b u4.g.n=%b", cm, u4.m, u4.g.n);
    $display("tri0 formal: z=%b %v u5.t=%b", z, z, u5.t);
    $display("tri1 actual: p=%b u6.o=%b %v", p, u6.o, u6.o);
    $display("supply0 formal: s=%b %v u7.s=%b", s, s, u7.s);
    $display("wand formal on wor actual: wo=%b u8.a=%b", wo, u8.a);
    force u0.n = 1'b1;
    #1 $display("forced formal: c=%b u0.n=%b", c, u0.n);
    release u0.n;
    #1 $display("released: c=%b u0.n=%b", c, u0.n);
    x = 1'b0;
    #1 $display("x=0: c=%b u0.n=%b m=%b u1.o=%b", c, u0.n, m, u1.o);
    $finish;
  end
endmodule
