// Decision S14-D3: `->>` with a timing control triggers the event that its
// operand names when the statement executes; rebinding the operand's event
// variable afterwards does not redirect the pending trigger.
//
// IEEE 1800-2009 15.5.1 (SystemVerilog-1800-2009.txt L20708-20711): "The
//   effect of the ->> operator is that the statement executes without
//   blocking and it creates a nonblocking assign update event in the time in
//   which the delay control expires or the event control occurs. The effect
//   of this update event shall be to trigger the referenced event in the
//   nonblocking assignment region of the simulation cycle."
// 10.4.2 (L13070-13071), for nonblocking assignments: "If variable_lvalue
//   requires an evaluation, it shall be evaluated at the same time as the
//   expression on the right-hand side."
//
// 15.5.1 does not say when "the referenced event" is resolved; llg resolves
// it at issue, like the target of a nonblocking assignment.
// Expected: x (named by h at 1) occurs at 4 after two ticks, y (named by h
// at 5) at 6; neither later binding of h is triggered.
`timescale 1ns / 1ns
module tb;
  event tick, x, y, h;

  initial forever #2 ->tick;

  always @x $display("%0t x", $time);
  always @y $display("%0t y", $time);

  initial begin
    #1 h = x;
    ->> repeat (2) @tick h;
    h = y;
    #4 h = y;
    ->> #1 h;
    h = x;
    #3 $finish;
  end
endmodule
