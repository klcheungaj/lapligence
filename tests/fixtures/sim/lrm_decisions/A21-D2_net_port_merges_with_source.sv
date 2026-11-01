// Decision A21-D2: a port whose sink is a wire, tri or logic net that the
// port alone drives (an input formal, or an output actual) is merged with
// its source into one simulated net. The sink shows the source's value in
// the same time step, the source's strength (a pulled actual is pulled on
// the formal too), and a force on a sink merged with a source net forces
// that net under both names. A net sink connected to a variable is not a net
// merge: a force on it does not reach the variable.
//
// IEEE 1800-2009 23.3.3 (SystemVerilog-1800-2009.txt L41452-41454):
//   "Each port connection shall be a continuous assignment of source to sink,
//   where one connected item shall be a signal source and the other shall be a
//   signal sink. The assignment shall be a continuous assignment from source
//   to sink for input or output ports."
// IEEE 1800-2009 23.3.3.7 (L41592-41598):
//   "When different net types are connected through a module port, the nets on
//   both sides of the port can take on the same type. [...] It is permissible
//   to merge the dominating and dominated nets into a single net, whose type
//   shall be that of the dominating net. The resulting net is called the
//   simulated net, and the dominated net is called a collapsed net."
// IEEE 1800-2009 10.6.2 (L13397-13399):
//   "A force procedural statement on a net shall override all drivers of the
//   net-gate outputs, module outputs, and continuous assignments-until a
//   release procedural statement is executed on the net. When released, the
//   net shall immediately be assigned the value determined by the drivers of
//   the net."
//
// The text allows both a continuous assignment, which drives the sink with
// its own (strong) strength one update later, and a merged net. llg follows
// the industry-standard behaviour and merges.
module child(input wire n, input wire p, input wire f, input wire g);
endmodule

module drv(output wire o, input logic d);
  assign o = d;
endmodule

module tb;
  logic x, d, v;
  wire w, fw, o;
  wire pu;
  assign w = x;
  assign fw = x;
  pullup (pu);
  child u(.n(w), .p(pu), .f(fw), .g(v));
  drv k(.o(o), .d(d));
  always @(w) if (w === 1'b1) $display("same step: w=%b u.n=%b", w, u.n);
  always @(k.o) if (k.o === 1'b1) $display("output sink: k.o=%b o=%b", k.o, o);
  initial begin
    x = 1'b0;
    v = 1'b0;
    d = 1'b0;
    #1 x = 1'b1;
    d = 1'b1;
    #1 $display("strength through the port: pu=%v u.p=%v", pu, u.p);
    force u.f = 1'b0;
    force u.g = 1'b1;
    #1 $display("forced net formal: fw=%b u.f=%b", fw, u.f);
    $display("forced net formal of a variable: v=%b u.g=%b", v, u.g);
    release u.f;
    release u.g;
    #1 $display("released: fw=%b u.f=%b v=%b u.g=%b", fw, u.f, v, u.g);
    $finish;
  end
endmodule
