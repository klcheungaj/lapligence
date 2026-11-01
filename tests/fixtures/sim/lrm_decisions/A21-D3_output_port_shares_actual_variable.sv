// Decision A21-D3: an output port whose formal is a variable and whose actual
// is a whole variable of the same type that nothing else writes shares the
// formal's storage: a process that writes the formal reads the new value
// through the actual's name at once, in the same time step. A force on the
// actual does not reach the formal; on release the actual takes the formal's
// value again, as the port's continuous assignment requires.
//
// IEEE 1800-2009 23.3.3 (SystemVerilog-1800-2009.txt L41452-41454):
//   "Each port connection shall be a continuous assignment of source to sink,
//   where one connected item shall be a signal source and the other shall be a
//   signal sink. The assignment shall be a continuous assignment from source
//   to sink for input or output ports."
// IEEE 1800-2009 23.3.3.2 (L41483-41486):
//   "An output port can be connected to a variable (or a concatenation) of a
//   compatible data type. A continuous assignment shall be implied when a
//   variable is connected to the output port of an instance. Procedural or
//   continuous assignments to a variable connected to the output port of an
//   instance shall be illegal."
// IEEE 1800-2009 10.6.2 (L13388-13393):
//   "A force statement to a variable shall override a procedural assignment,
//   continuous assignment or an assign procedural continuous assignment to the
//   variable until a release procedural statement is executed on the
//   variable. [...] Releasing a variable that is driven by a continuous
//   assignment or currently has an active assign procedural continuous
//   assignment shall reestablish that assignment and schedule a reevaluation
//   in the continuous assignment's scheduling region."
//
// The text describes the port as a continuous assignment; whether the actual
// shows the new value inside the writing process is not stated. llg follows
// the industry-standard behaviour: the actual shows it.
module child(input logic clk, output logic q, output logic r);
  initial begin
    q = 1'b0;
    r = 1'b0;
  end
  always @(posedge clk) begin
    q = ~q;
    r = 1'b1;
    $display("child wrote q=%b, actual y=%b", q, tb.y);
  end
endmodule

module tb;
  logic clk, y, yr;
  child u(.clk(clk), .q(y), .r(yr));
  initial begin
    clk = 1'b0;
    #1 clk = 1'b1;
    #1 force yr = 1'b0;
    #1 $display("forced actual: yr=%b u.r=%b", yr, u.r);
    release yr;
    #1 $display("released actual: yr=%b u.r=%b", yr, u.r);
    $finish;
  end
endmodule
