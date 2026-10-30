// Decision A21-D1: an input port whose formal is a variable (an `input logic`
// port is one) and whose actual is a whole variable of the same type shares
// the actual's storage: the two names are one simulated object. A process
// that writes the actual reads the new value through the formal at once, in
// the same time step, without an intervening port-connection update.
// Where sharing would be observable it is not done and the port stays a
// continuous assignment: a force on the formal leaves the actual alone, and
// when the actual has a declaration initializer a process already waiting on
// the formal at time 0 sees the formal change from its own default (the
// initializer itself is no change, T0-D1). A force on the actual reaches the
// formal either way.
//
// IEEE 1800-2009 4.9.6 (SystemVerilog-1800-2009.txt L3578-3580):
//   "Ports can always be represented as declared objects connected, as
//   follows: — If an input port, then a continuous assignment from an outside
//   expression to a local (input) net or variable"
// IEEE 1800-2009 23.3.3 (L41452-41454):
//   "Each port connection shall be a continuous assignment of source to sink,
//   where one connected item shall be a signal source and the other shall be a
//   signal sink. The assignment shall be a continuous assignment from source
//   to sink for input or output ports."
// IEEE 1800-2009 23.3.3.7 (L41596-41598):
//   "It is permissible to merge the dominating and dominated nets into a single
//   net, whose type shall be that of the dominating net. The resulting net is
//   called the simulated net, and the dominated net is called a collapsed net."
// IEEE 1800-2009 10.6.2 (L13388-13390):
//   "A force statement to a variable shall override a procedural assignment,
//   continuous assignment or an assign procedural continuous assignment to the
//   variable until a release procedural statement is executed on the
//   variable."
//
// The text describes the port as a continuous assignment and permits merging
// for nets only; whether the formal shows the new value inside the writing
// process is not stated. llg follows the industry-standard behaviour: the
// formal shows it.
module child(input logic a, input logic [3:0] v, input logic f, input logic t0);
  bit seen_t0;
  always @(t0) seen_t0 = 1'b1;
endmodule

module tb;
  logic x;
  logic [3:0] y;
  logic z;
  logic init = 1'b1;
  child u(.a(x), .v(y), .f(z), .t0(init));
  initial begin
    x = 1'b0;
    y = 4'd0;
    z = 1'b0;
    #1;
    x = 1'b1;
    y = 4'd9;
    $display("same step: u.a=%b u.v=%0d", u.a, u.v);
    #1 force u.f = 1'b0;
    z = 1'b1;
    #1 $display("formal forced: z=%b u.f=%b", z, u.f);
    release u.f;
    #1 $display("formal released: z=%b u.f=%b", z, u.f);
    force x = 1'b0;
    #1 $display("actual forced: x=%b u.a=%b", x, u.a);
    release x;
    $display("time-0 change of an initialized actual: %0d", u.seen_t0);
    $finish;
  end
endmodule
