// Decision A13-D8: a property local with a declaration assignment in a
// property with two semantic leading clocks has one copy per leading clock,
// and each copy's initialization reads the sampled values at that clock's
// first tick at or after the start of the evaluation. In
// @(posedge c1) e |=> two_copies the instance starts after the c1 tick 10:
// the c2 copy is initialized at the c2 tick 16 (x = 3) and the c1 copy at the
// c1 tick 40 (x = 4), so both checks hold and the attempt passes at 40.
// Initializing both copies with the Preponed values of the time step where
// the attempt began (x = 0 at 10) would fail.
//
// IEEE 1800-2009 16.14.7 (SystemVerilog-1800-2009.txt L26329-26335): "If
// there are two or more distinct semantic leading clocks for an instance of
// a named property, then a separate copy of the local variable shall be
// created for each semantic leading clock. For each copy of the local
// variable, the initialization assignment shall be performed at the earliest
// tick of the corresponding semantic leading clock that is at or after the
// beginning of the evaluation attempt". F.4.2 (L70939-70940) pushes the
// assignment to `kappa(r) (1, E) ##0 (r)`, evaluated at that tick. 16.10
// (L23805-23807) reads non-local variables of an initialization "using the
// Preponed values from the time slot in which the evaluation attempt
// begins"; llg follows 16.14.7, F.4.2 and the industry-standard behaviour.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, e = 1'b0;
  logic [2:0] x = 3'd0;
  property two_copies;
    logic [2:0] v = x;
    (@(posedge c1) v == 3'd4) and (@(posedge c2) v == 3'd3);
  endproperty
  initial p1: assert property (@(posedge c1) e |=> two_copies)
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  initial begin
    #8 e = 1'b1;
    #2 c1 = 1'b1;
    #2 e = 1'b0;
    #1 x = 3'd3;
    #3 c2 = 1'b1;
    #7 x = 3'd4;
    #7 c1 = 1'b0;
    #10 c1 = 1'b1;
    #10 $finish;
  end
endmodule
